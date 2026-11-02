use super::*;

const ANSI_ACCENT: &str = crate::terminal_theme::ANSI_ACCENT;
const ANSI_BLUE: &str = crate::terminal_theme::ANSI_BLUE;
const ANSI_ACCENT_BOLD: &str = crate::terminal_theme::ANSI_ACCENT_BOLD;
const ANSI_MUTED: &str = crate::terminal_theme::ANSI_MUTED;
const ANSI_WARNING: &str = crate::terminal_theme::ANSI_WARNING;
const ANSI_ERROR: &str = crate::terminal_theme::ANSI_ERROR;
const ANSI_BOLD: &str = crate::terminal_theme::ANSI_BOLD;

/// Kept deliberately short (not spanning the full terminal width): the
/// already-ANSI-colored line gets re-wrapped by `textwrap` in
/// `render_live_summary`, which counts escape-code bytes toward width, so a
/// long border risks being cut mid-line. A short border avoids that.
pub(super) fn code_block_border_dashes(term_width: usize) -> usize {
    term_width.saturating_sub(10).clamp(16, 40)
}

pub(super) fn code_block_top_border(lang: &str, term_width: usize) -> String {
    let dashes = code_block_border_dashes(term_width);
    let label = if lang.is_empty() {
        String::new()
    } else {
        format!("─ {lang} ")
    };
    format!("{CODE_BORDER}┌{label}{}{RESET}", "─".repeat(dashes))
}

pub(super) fn code_block_bottom_border(term_width: usize) -> String {
    let dashes = code_block_border_dashes(term_width);
    format!("{CODE_BORDER}└{}{RESET}", "─".repeat(dashes))
}

/// Loading these involves parsing bundled `.sublime-syntax`/theme data, so it's done once
/// per process (on first code block rendered) rather than on every `format_markdown_bold`
/// call — a chatty agent turn can print many code blocks in one response.
static SYNTAX_SET: std::sync::LazyLock<SyntaxSet> =
    std::sync::LazyLock::new(SyntaxSet::load_defaults_newlines);
static THEME_SET: std::sync::LazyLock<ThemeSet> = std::sync::LazyLock::new(ThemeSet::load_defaults);

/// Starts a highlighter for `lang` (a fenced code block's language hint, e.g. `rust`,
/// `ts`, `py` — `find_syntax_by_token` already knows the common short aliases). Returns
/// `None` for an empty/unrecognized hint, in which case the caller falls back to
/// unhighlighted code — better than guessing wrong and coloring things incorrectly.
pub(super) fn start_code_highlighter(lang: &str) -> Option<HighlightLines<'static>> {
    let lang = lang.trim();
    if lang.is_empty() {
        return None;
    }
    let syntax = SYNTAX_SET.find_syntax_by_token(lang)?;
    let theme = &THEME_SET.themes[crate::terminal_theme::SYNTAX_THEME];
    Some(HighlightLines::new(syntax, theme))
}

/// Highlights one code-block line and returns it ANSI-colored, ready to print. Falls back
/// to the plain line on any error rather than dropping content — a rendering glitch should
/// never be the reason a line of the agent's actual answer goes missing.
pub(super) fn highlight_code_line(highlighter: &mut HighlightLines, line: &str) -> String {
    // syntect's line-oriented highlighter tracks state (e.g. "inside a string") across
    // calls and expects each line to end in `\n` for that state tracking to be accurate,
    // even though the trailing newline itself isn't meaningful here.
    let with_newline = format!("{line}\n");
    match highlighter.highlight_line(&with_newline, &SYNTAX_SET) {
        Ok(ranges) => as_24_bit_terminal_escaped(&ranges, false)
            .trim_end_matches('\n')
            .to_string(),
        Err(_) => line.to_string(),
    }
}

fn render_cli_ui_grid(json_str: &str, term_width: usize) -> Option<Vec<String>> {
    let val: serde_json::Value = serde_json::from_str(json_str).ok()?;
    let items = if let Some(arr) = val.as_array() {
        arr.clone()
    } else if let Some(items_arr) = val.get("items").and_then(|v| v.as_array()) {
        items_arr.clone()
    } else {
        return None;
    };
    if items.is_empty() {
        return None;
    }

    let dashes = term_width.saturating_sub(14).clamp(16, 50);
    let title = val
        .get("title")
        .and_then(|v| v.as_str())
        .unwrap_or("OPTIONS");
    let mut lines = Vec::new();
    lines.push(format!(
        "{ANSI_BLUE}┌─ {title} {}{RESET}",
        "─".repeat(dashes)
    ));

    if let Some(subtitle) = val.get("subtitle").and_then(|v| v.as_str()) {
        lines.push(format!("{ANSI_BLUE}│{RESET} {ANSI_MUTED}{subtitle}{RESET}"));
        lines.push(format!("{ANSI_BLUE}│{RESET}"));
    }

    for (idx, item) in items.iter().enumerate() {
        let item_title = item.get("title").and_then(|v| v.as_str()).unwrap_or("");
        let item_desc = item.get("desc").and_then(|v| v.as_str()).unwrap_or("");
        let item_badge = item.get("badge").and_then(|v| v.as_str());

        let badge_str = if let Some(b) = item_badge {
            format!(" {ANSI_ACCENT_BOLD}[{b}]{RESET}")
        } else {
            String::new()
        };

        lines.push(format!(
            "{ANSI_BLUE}│{RESET} {ANSI_BLUE}[◆]{RESET} {ANSI_BOLD}{item_title}{RESET}{badge_str}"
        ));
        if !item_desc.is_empty() {
            lines.push(format!(
                "{ANSI_BLUE}│{RESET}     {ANSI_MUTED}{item_desc}{RESET}"
            ));
        }
        if idx < items.len() - 1 {
            lines.push(format!("{ANSI_BLUE}│{RESET}"));
        }
    }

    lines.push(format!("{ANSI_BLUE}└{}{RESET}", "─".repeat(dashes + 4)));
    Some(lines)
}

fn render_cli_ui_card(json_str: &str, term_width: usize) -> Option<Vec<String>> {
    let val: serde_json::Value = serde_json::from_str(json_str).ok()?;
    let items: Vec<serde_json::Value> = if let Some(arr) = val.as_array() {
        arr.clone()
    } else if let Some(items_arr) = val.get("items").and_then(|v| v.as_array()) {
        items_arr.clone()
    } else {
        vec![val.clone()]
    };
    if items.is_empty() {
        return None;
    }

    let dashes = term_width.saturating_sub(14).clamp(16, 50);
    let mut lines = Vec::new();

    for (idx, item) in items.iter().enumerate() {
        let title = item
            .get("title")
            .and_then(|v| v.as_str())
            .unwrap_or("FEATURE");
        let subtitle = item.get("subtitle").and_then(|v| v.as_str());
        let badge = item.get("badge").and_then(|v| v.as_str());

        let badge_str = if let Some(b) = badge {
            format!(" {ANSI_ACCENT_BOLD}[{b}]{RESET}")
        } else {
            String::new()
        };

        lines.push(format!(
            "{ANSI_ACCENT}┌─ {ANSI_BOLD}{title}{RESET}{badge_str} {}{RESET}",
            "─".repeat(dashes.saturating_sub(title.len() + 2))
        ));

        if let Some(sub) = subtitle {
            lines.push(format!("{ANSI_ACCENT}│{RESET} {sub}"));
        }

        if let Some(details) = item.get("details").and_then(|v| v.as_object()) {
            for (k, v) in details {
                let val_str = v.as_str().unwrap_or("");
                lines.push(format!(
                    "{ANSI_ACCENT}│{RESET}   {ANSI_BOLD}{k}:{RESET} {val_str}"
                ));
            }
        }

        lines.push(format!("{ANSI_ACCENT}└{}{RESET}", "─".repeat(dashes + 4)));
        if idx < items.len() - 1 {
            lines.push(String::new());
        }
    }

    Some(lines)
}

fn render_cli_ui_mockup(json_str: &str, term_width: usize) -> Option<Vec<String>> {
    let val: serde_json::Value = serde_json::from_str(json_str).ok()?;
    let title = val
        .get("title")
        .and_then(|v| v.as_str())
        .unwrap_or("Mockup");
    let subtitle = val.get("subtitle").and_then(|v| v.as_str());
    let dropzone_text = val
        .get("dropzoneText")
        .and_then(|v| v.as_str())
        .unwrap_or("Drop file here");
    let disclaimer = val
        .get("disclaimer")
        .and_then(|v| v.as_str())
        .unwrap_or("UI Concept Only");

    let dashes = term_width.saturating_sub(14).clamp(16, 50);
    let mut lines = Vec::new();

    lines.push(format!(
        "{ANSI_BLUE}┌─ [MOCKUP] {ANSI_BOLD}{title}{RESET} {}{RESET}",
        "─".repeat(dashes.saturating_sub(title.len() + 10))
    ));

    if let Some(sub) = subtitle {
        lines.push(format!("{ANSI_BLUE}│{RESET} {ANSI_MUTED}{sub}{RESET}"));
    }

    lines.push(format!(
        "{ANSI_BLUE}│{RESET}  {ANSI_MUTED}┌─ Dropzone ────────────────────────┐{RESET}"
    ));
    lines.push(format!("{ANSI_BLUE}│{RESET}  {ANSI_MUTED}│{RESET} {ANSI_BLUE}[↑]{RESET} {dropzone_text}         {ANSI_MUTED}│{RESET}"));
    lines.push(format!(
        "{ANSI_BLUE}│{RESET}  {ANSI_MUTED}└───────────────────────────────────┘{RESET}"
    ));

    if let Some(metrics) = val.get("metrics").and_then(|v| v.as_object()) {
        for (k, v) in metrics {
            let val_str = v.as_str().unwrap_or("");
            lines.push(format!(
                "{ANSI_BLUE}│{RESET}   {ANSI_MUTED}{k}:{RESET} {ANSI_BOLD}{val_str}{RESET}"
            ));
        }
    }

    lines.push(format!(
        "{ANSI_BLUE}│{RESET}  {ANSI_MUTED}ℹ {disclaimer}{RESET}"
    ));
    lines.push(format!("{ANSI_BLUE}└{}{RESET}", "─".repeat(dashes + 4)));

    Some(lines)
}

fn try_render_single_line_custom_block(raw: &str, term_width: usize) -> Option<Vec<String>> {
    let raw = raw.trim();
    for prefix in &["ui-grid", "ui_grid", "ui-grid-json", "ui_grid_json"] {
        if let Some(rest) = raw.strip_prefix(prefix) {
            let json = rest.trim();
            if json.starts_with('[') || json.starts_with('{') {
                return render_cli_ui_grid(json, term_width);
            }
        }
    }
    for prefix in &[
        "ui-card",
        "ui_card",
        "ui-card-json",
        "ui_card_json",
        "ui-feature",
        "ui_feature",
    ] {
        if let Some(rest) = raw.strip_prefix(prefix) {
            let json = rest.trim();
            if json.starts_with('[') || json.starts_with('{') {
                return render_cli_ui_card(json, term_width);
            }
        }
    }
    for prefix in &["ui-mockup", "ui_mockup", "ui-mockup-json", "ui_mockup_json"] {
        if let Some(rest) = raw.strip_prefix(prefix) {
            let json = rest.trim();
            if json.starts_with('{') || json.starts_with('[') {
                return render_cli_ui_mockup(json, term_width);
            }
        }
    }
    None
}

pub(crate) fn is_atomic_fenced_lang(lang: &str) -> bool {
    matches!(
        lang.to_ascii_lowercase().as_str(),
        "ui-grid"
            | "ui_grid"
            | "ui-grid-json"
            | "ui_grid_json"
            | "ui-card"
            | "ui_card"
            | "ui-card-json"
            | "ui_card_json"
            | "ui-feature"
            | "ui_feature"
            | "ui-mockup"
            | "ui_mockup"
            | "ui-mockup-json"
            | "ui_mockup_json"
            | "weather_json"
            | "weather-json"
            | "stock_json"
            | "stock-json"
            | "calculation_json"
            | "calculation-json"
            | "image_search_json"
            | "image-search-json"
            | "image_gen_json"
            | "image-gen-json"
            | "mermaid"
    )
}

/// Splits Markdown into progressive reveal units:
/// - Regular text, headings, lists, callouts, and standard code blocks split line-by-line.
/// - Markdown tables (`| ... |`) group all consecutive rows into a single atomic unit.
/// - Custom UI/JSON/Mermaid fenced blocks (`ui-card`, `ui-grid`, `ui-mockup`, etc.) group the
///   entire fenced block into a single atomic unit so raw JSON never flashes mid-stream.
///
/// When `is_complete` is `false`, trailing incomplete lines, open tables, and unclosed atomic
/// fenced blocks are held back until completed.
pub(crate) fn split_progressive_units(raw: &str, is_complete: bool) -> Vec<String> {
    if raw.is_empty() {
        return Vec::new();
    }
    let slice = if is_complete || raw.ends_with('\n') {
        raw
    } else if let Some(last_nl) = raw.rfind('\n') {
        &raw[..=last_nl]
    } else {
        return Vec::new();
    };

    let mut units = Vec::new();
    let mut in_code_block = false;
    let mut pending_code_fence: Option<(String, bool)> = None;
    let mut in_atomic_fence = false;
    let mut atomic_fence_buffer: Vec<String> = Vec::new();
    let mut table_buffer: Vec<String> = Vec::new();

    for raw_line in slice.lines() {
        let line = raw_line.strip_suffix('\r').unwrap_or(raw_line);
        let trimmed = line.trim_start();

        if !in_code_block && !in_atomic_fence && markdown::is_table_line(trimmed) {
            table_buffer.push(line.to_owned());
            continue;
        }

        if !table_buffer.is_empty() {
            units.push(table_buffer.join("\n"));
            table_buffer.clear();
        }

        if trimmed.starts_with("```") {
            if !in_code_block && !in_atomic_fence {
                let after_open = trimmed.trim_start_matches('`').trim();
                if after_open.ends_with("```") && after_open.len() > 3 {
                    units.push(line.to_owned());
                    continue;
                }
                let lang = if let Some(idx) =
                    after_open.find(|c: char| c.is_whitespace() || c == '[' || c == '{')
                {
                    &after_open[..idx]
                } else {
                    after_open
                };
                if is_atomic_fenced_lang(lang) {
                    in_atomic_fence = true;
                    atomic_fence_buffer.clear();
                    atomic_fence_buffer.push(line.to_owned());
                } else {
                    in_code_block = true;
                    let can_promote = lang.is_empty() || lang.eq_ignore_ascii_case("plaintext");
                    pending_code_fence = Some((line.to_owned(), can_promote));
                }
            } else if in_atomic_fence {
                atomic_fence_buffer.push(line.to_owned());
                units.push(atomic_fence_buffer.join("\n"));
                atomic_fence_buffer.clear();
                in_atomic_fence = false;
            } else {
                in_code_block = false;
                if let Some((fence, _)) = pending_code_fence.take() {
                    units.push(format!("{fence}\n{line}"));
                } else {
                    units.push(line.to_owned());
                }
            }
            continue;
        }

        if in_atomic_fence {
            atomic_fence_buffer.push(line.to_owned());
            continue;
        }

        if let Some((fence, can_promote)) = pending_code_fence.take() {
            let first_token = trimmed
                .find(|c: char| c.is_whitespace() || c == '[' || c == '{')
                .map_or(trimmed, |idx| &trimmed[..idx]);
            if can_promote && is_atomic_fenced_lang(first_token) {
                in_code_block = false;
                in_atomic_fence = true;
                atomic_fence_buffer.clear();
                atomic_fence_buffer.push(fence);
                atomic_fence_buffer.push(line.to_owned());
            } else {
                units.push(format!("{fence}\n{line}"));
            }
            continue;
        }

        units.push(line.to_owned());
    }

    if is_complete {
        if !table_buffer.is_empty() {
            units.push(table_buffer.join("\n"));
        }
        if let Some((fence, _)) = pending_code_fence.take() {
            units.push(fence);
        }
        if !atomic_fence_buffer.is_empty() {
            units.push(atomic_fence_buffer.join("\n"));
        }
    }

    units
}

#[derive(Debug, Default, Clone)]
pub(crate) struct ProgressiveMarkdownPacer {
    raw_buffer: String,
    completed: bool,
    revealed_units: usize,
}

impl ProgressiveMarkdownPacer {
    pub fn push_delta(&mut self, delta: &str) {
        if !delta.is_empty() {
            self.raw_buffer.push_str(delta);
        }
    }

    pub fn finish_with_authoritative(&mut self, full_text: &str) {
        self.raw_buffer.clear();
        self.raw_buffer.push_str(full_text);
        self.completed = true;
    }

    pub fn is_completed(&self) -> bool {
        self.completed
    }

    pub fn is_done(&self) -> bool {
        if !self.completed {
            return false;
        }
        let units = split_progressive_units(&self.raw_buffer, true);
        self.revealed_units >= units.len()
    }

    pub fn step(&mut self) -> Option<String> {
        let units = split_progressive_units(&self.raw_buffer, self.completed);
        if self.revealed_units >= units.len() {
            return None;
        }
        let remaining = units.len() - self.revealed_units;
        let advance = if remaining <= 12 {
            1
        } else {
            (remaining / 10).clamp(1, 4)
        };
        self.revealed_units = (self.revealed_units + advance).min(units.len());
        Some(units[..self.revealed_units].join("\n"))
    }

    pub fn flush_all(&mut self) -> Option<String> {
        self.completed = true;
        let units = split_progressive_units(&self.raw_buffer, true);
        if units.is_empty() {
            return None;
        }
        self.revealed_units = units.len();
        Some(units.join("\n"))
    }
}

pub(crate) fn format_markdown_bold(text: &str) -> String {
    let (term_width, _) = markdown::terminal_size_or_default();
    format_markdown_bold_with_width(text, term_width as usize)
}

pub(crate) fn render_markdown_to_lines(
    text: &str,
    term_width: usize,
) -> (Vec<ratatui::text::Line<'static>>, Vec<String>) {
    if text.is_empty() {
        return (Vec::new(), Vec::new());
    }
    let processed = format_markdown_bold_with_width(text, term_width.max(20));
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
        let rendered: Vec<ratatui::text::Line<'static>> = text
            .lines()
            .map(|l| ratatui::text::Line::raw(l.to_string()))
            .collect();
        let plain: Vec<String> = text.lines().map(str::to_owned).collect();
        (rendered, plain)
    }
}

pub(crate) fn format_markdown_bold_with_width(text: &str, term_width: usize) -> String {
    let term_width = term_width.max(20);

    let mut formatted_lines = Vec::new();
    let mut in_code_block = false;
    let mut highlighter: Option<HighlightLines> = None;
    let mut active_custom_block: Option<String> = None;
    let mut custom_block_buffer: Vec<String> = Vec::new();
    let mut active_alert_color: Option<&'static str> = None;
    // Markdown table buffering: collect consecutive `|...|` lines so we can
    // flush them through `render_markdown_table_with_width` as a unit (same as classic
    // mode in `render_live_summary`).
    let mut table_buffer: Vec<String> = Vec::new();

    // Flush any buffered table rows into `formatted_lines`.
    macro_rules! flush_table {
        () => {
            if !table_buffer.is_empty() {
                let rendered = markdown::render_markdown_table_with_width(
                    &table_buffer,
                    term_width.min(u16::MAX as usize) as u16,
                );
                for tl in rendered.split('\n') {
                    formatted_lines.push(tl.to_string());
                }
                table_buffer.clear();
            }
        };
    }

    for line in text.lines() {
        let mut formatted_line = line.to_string();
        let trimmed = line.trim_start();

        // Markdown table rows (`|...|`) — buffer them and let
        // `render_markdown_table` produce the box-drawing output once the
        // table ends.  Skip this detection inside code blocks to avoid
        // treating table-like literal source code as a table.
        if !in_code_block && markdown::is_table_line(trimmed) {
            table_buffer.push(line.to_string());
            continue;
        }
        // Non-table line: flush any buffered table rows first.
        flush_table!();

        // Check for single-line custom UI block wrapped in backticks or bare
        if !in_code_block {
            let stripped = trimmed.trim_matches('`').trim();
            if let Some(lines) = try_render_single_line_custom_block(stripped, term_width) {
                formatted_lines.extend(lines);
                continue;
            }
        }

        if trimmed.starts_with("```") {
            let was_in_code_block = in_code_block;
            in_code_block = !in_code_block;
            if was_in_code_block {
                highlighter = None;
                if let Some(custom_lang) = active_custom_block.take() {
                    let buffered = custom_block_buffer.join("\n");
                    custom_block_buffer.clear();
                    let rendered = match custom_lang.as_str() {
                        "ui-grid" | "ui_grid" | "ui-grid-json" | "ui_grid_json" => {
                            render_cli_ui_grid(&buffered, term_width)
                        }
                        "ui-card" | "ui_card" | "ui-card-json" | "ui_card_json" | "ui-feature"
                        | "ui_feature" => render_cli_ui_card(&buffered, term_width),
                        "ui-mockup" | "ui_mockup" | "ui-mockup-json" | "ui_mockup_json" => {
                            render_cli_ui_mockup(&buffered, term_width)
                        }
                        _ => None,
                    };
                    if let Some(lines) = rendered {
                        formatted_lines.extend(lines);
                    } else {
                        formatted_lines.push(code_block_top_border(&custom_lang, term_width));
                        for l in buffered.lines() {
                            formatted_line = format!("  {l}");
                            formatted_lines.push(formatted_line);
                        }
                        formatted_lines.push(code_block_bottom_border(term_width));
                    }
                } else {
                    formatted_lines.push(code_block_bottom_border(term_width));
                }
            } else {
                let lang_raw = trimmed.trim_start_matches('`').trim();
                let (lang, rest) = if let Some(space_idx) =
                    lang_raw.find(|c: char| c.is_whitespace() || c == '[' || c == '{')
                {
                    (&lang_raw[..space_idx], lang_raw[space_idx..].trim())
                } else {
                    (lang_raw, "")
                };

                match lang {
                    "ui-grid" | "ui_grid" | "ui-grid-json" | "ui_grid_json" | "ui-card"
                    | "ui_card" | "ui-card-json" | "ui_card_json" | "ui-feature" | "ui_feature"
                    | "ui-mockup" | "ui_mockup" | "ui-mockup-json" | "ui_mockup_json" => {
                        active_custom_block = Some(lang.to_string());
                        custom_block_buffer.clear();
                        if !rest.is_empty() {
                            custom_block_buffer.push(rest.to_string());
                        }
                    }
                    _ => {
                        active_custom_block = None;
                        formatted_lines.push(code_block_top_border(lang, term_width));
                        highlighter = start_code_highlighter(lang);
                    }
                }
            }
            continue;
        }

        if in_code_block {
            if active_custom_block.is_some() {
                custom_block_buffer.push(line.to_string());
                continue;
            }
            // Normal code block
            let content = match highlighter.as_mut() {
                Some(h) => highlight_code_line(h, line),
                None => line.to_string(),
            };
            formatted_line = format!("  {content}{RESET}");
            formatted_lines.push(formatted_line);
            continue;
        }

        // Horizontal rules (---, ***, ___)
        let is_hr = (trimmed.starts_with("---") && trimmed.chars().all(|c| c == '-'))
            || (trimmed.starts_with("***") && trimmed.chars().all(|c| c == '*'))
            || (trimmed.starts_with("___") && trimmed.chars().all(|c| c == '_'));
        if is_hr && trimmed.len() >= 3 {
            active_alert_color = None;
            let hr_width = term_width.clamp(20, 50);
            formatted_lines.push(format!("{ANSI_MUTED}{}{RESET}", "─".repeat(hr_width)));
            continue;
        }

        // Handle blockquotes / GitHub alerts (both with and without leading '>')
        let is_quote = trimmed.starts_with('>');
        let is_standalone_alert = trimmed.starts_with("[!");
        if is_quote || is_standalone_alert {
            let quote_content = if is_quote {
                trimmed[1..].trim()
            } else {
                trimmed
            };
            let upper = quote_content.to_uppercase();
            if upper.starts_with("[!NOTE]") {
                active_alert_color = Some(crate::terminal_theme::ANSI_BLUE);
                let body = quote_content[7..].trim();
                if body.is_empty() {
                    formatted_lines.push(format!(
                        "{}│ NOTE:{RESET}",
                        crate::terminal_theme::ANSI_BLUE
                    ));
                } else {
                    formatted_lines.push(format!(
                        "{}│ NOTE:{RESET} {}",
                        crate::terminal_theme::ANSI_BLUE,
                        process_inline_bold(body)
                    ));
                }
                continue;
            } else if upper.starts_with("[!TIP]") {
                active_alert_color = Some(ANSI_ACCENT);
                let body = quote_content[6..].trim();
                if body.is_empty() {
                    formatted_lines.push(format!("{ANSI_ACCENT}│ TIP:{RESET}"));
                } else {
                    formatted_lines.push(format!(
                        "{ANSI_ACCENT}│ TIP:{RESET} {}",
                        process_inline_bold(body)
                    ));
                }
                continue;
            } else if upper.starts_with("[!IMPORTANT]") {
                active_alert_color = Some(ANSI_ACCENT);
                let body = quote_content[12..].trim();
                if body.is_empty() {
                    formatted_lines.push(format!("{ANSI_ACCENT}│ IMPORTANT:{RESET}"));
                } else {
                    formatted_lines.push(format!(
                        "{ANSI_ACCENT}│ IMPORTANT:{RESET} {}",
                        process_inline_bold(body)
                    ));
                }
                continue;
            } else if upper.starts_with("[!WARNING]") {
                active_alert_color = Some(ANSI_WARNING);
                let body = quote_content[10..].trim();
                if body.is_empty() {
                    formatted_lines.push(format!("{ANSI_WARNING}│ WARNING:{RESET}"));
                } else {
                    formatted_lines.push(format!(
                        "{ANSI_WARNING}│ WARNING:{RESET} {}",
                        process_inline_bold(body)
                    ));
                }
                continue;
            } else if upper.starts_with("[!CAUTION]") {
                active_alert_color = Some(ANSI_ERROR);
                let body = quote_content[10..].trim();
                if body.is_empty() {
                    formatted_lines.push(format!("{ANSI_ERROR}│ CAUTION:{RESET}"));
                } else {
                    formatted_lines.push(format!(
                        "{ANSI_ERROR}│ CAUTION:{RESET} {}",
                        process_inline_bold(body)
                    ));
                }
                continue;
            } else if is_quote {
                if let Some(color) = active_alert_color {
                    formatted_lines.push(format!(
                        "{color}│{RESET} {}",
                        process_inline_bold(quote_content)
                    ));
                } else {
                    formatted_lines.push(format!(
                        "{ANSI_MUTED}│{RESET} \x1b[3m{}{RESET}",
                        process_inline_bold(quote_content)
                    ));
                }
                continue;
            }
        } else {
            active_alert_color = None;
        }

        let mut leading_spaces = 0;
        let mut is_list_item = false;
        let mut marker_char = '-';

        for (idx, c) in line.char_indices() {
            if c.is_whitespace() {
                leading_spaces += c.len_utf8();
            } else {
                if c == '-' || c == '*' || c == '+' {
                    let after = &line[idx + c.len_utf8()..];
                    if after.chars().next().is_some_and(|c| c.is_whitespace()) {
                        is_list_item = true;
                        marker_char = c;
                    }
                }
                break;
            }
        }

        if is_list_item {
            let marker_len = marker_char.len_utf8();
            let mut new_line = String::new();
            new_line.push_str(&line[..leading_spaces]);
            new_line.push_str(&format!("{ANSI_BLUE}•{RESET}"));
            new_line.push_str(&line[leading_spaces + marker_len..]);
            formatted_line = process_inline_bold(&new_line);
        } else {
            let hash_count = trimmed.chars().take_while(|&c| c == '#').count();
            let is_heading =
                (1..=6).contains(&hash_count) && trimmed.as_bytes().get(hash_count) == Some(&b' ');
            if is_heading {
                let leading_len = line.len() - trimmed.len();
                let leading_spaces_str = &line[..leading_len];
                let heading_text = trimmed[hash_count + 1..].trim_end();
                let (style_start, style_end) = match hash_count {
                    1 => (crate::terminal_theme::ANSI_BLUE_BOLD_UNDERLINE, RESET),
                    2 => (crate::terminal_theme::ANSI_BLUE_BOLD, RESET),
                    _ => (BRIGHT, RESET),
                };
                formatted_line = format!(
                    "{leading_spaces_str}{style_start}{}{style_end}",
                    process_inline_bold(heading_text)
                );
            } else {
                formatted_line = process_inline_bold(&formatted_line);
            }
        }

        formatted_lines.push(formatted_line);
    }

    // Flush any table that was still buffered at the end of the text.
    flush_table!();

    let mut result = formatted_lines.join("\n");
    if text.ends_with('\n') {
        result.push('\n');
    }
    result
}

fn process_inline_badges(text: &str) -> String {
    if !text.contains("[badge") {
        return text.to_string();
    }
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(start) = rest.find("[badge") {
        out.push_str(&rest[..start]);
        let after_start = &rest[start..];
        if let Some(end) = after_start.find(']') {
            let badge_content = &after_start[..=end];
            if badge_content.starts_with("[badge:") || badge_content.starts_with("[badge ") {
                let inner = badge_content[6..badge_content.len() - 1].trim();
                let (color, label) = if inner.starts_with(':') {
                    if let Some(space_pos) = inner.find(' ') {
                        (&inner[1..space_pos], inner[space_pos + 1..].trim())
                    } else {
                        ("green", inner)
                    }
                } else {
                    ("green", inner)
                };

                let fg = match color {
                    "blue" | "purple" => crate::terminal_theme::ANSI_BLUE_BOLD,

                    "amber" | "yellow" => crate::terminal_theme::ANSI_WARNING_BOLD,
                    "red" => crate::terminal_theme::ANSI_ERROR_BOLD,
                    _ => ANSI_ACCENT_BOLD,
                };

                out.push_str(&format!("{fg}{label}{RESET}"));
                rest = &after_start[end + 1..];
            } else {
                out.push_str(&after_start[..1]);
                rest = &after_start[1..];
            }
        } else {
            out.push_str(&after_start[..1]);
            rest = &after_start[1..];
        }
    }
    out.push_str(rest);
    out
}

pub(crate) fn process_inline_code(text: &str) -> String {
    if !text.contains('`') {
        return text.to_string();
    }
    let count = text.matches('`').count();
    let pair_limit = (count / 2) * 2;
    let mut result = String::with_capacity(text.len());
    let parts = text.split('`');
    let mut is_code = false;
    let mut processed_markers = 0;
    for part in parts {
        if is_code && processed_markers < pair_limit {
            result.push_str(CYAN);
            result.push_str(part);
            result.push_str(RESET);
        } else {
            result.push_str(part);
        }
        processed_markers += 1;
        is_code = !is_code;
    }
    result
}

pub(crate) fn process_inline_bold(text: &str) -> String {
    let count = text.matches("**").count();
    let pair_limit = (count / 2) * 2;
    let mut result = String::with_capacity(text.len());
    let parts = text.split("**");
    let mut is_bold = false;
    let mut processed_markers = 0;
    for part in parts {
        if is_bold && processed_markers < pair_limit {
            result.push_str(BOLD);
            result.push_str(BLUE);
            result.push_str(part);
            result.push_str(RESET);
        } else {
            result.push_str(part);
        }
        processed_markers += 1;
        is_bold = !is_bold;
    }
    let with_code = process_inline_code(&result);
    process_inline_badges(&with_code)
}

/// Replace common LaTeX math symbols with Unicode equivalents.
/// Fixes garbled output like "ightarrow$" from models that emit LaTeX notation.
pub(crate) fn sanitize_latex(text: &str) -> String {
    let mut s = text.to_owned();
    for (pat, uni) in [
        // arrows
        ("$\\rightarrow$", "→"),
        ("\\rightarrow", "→"),
        ("ightarrow", "→"),
        ("$\\leftarrow$", "←"),
        ("\\leftarrow", "←"),
        ("eftarrow", "←"),
        ("$\\Rightarrow$", "⇒"),
        ("\\Rightarrow", "⇒"),
        ("$\\Leftarrow$", "⇐"),
        ("\\Leftarrow$", "⇐"),
        ("$\\leftrightarrow$", "↔"),
        ("\\leftrightarrow", "↔"),
        // comparison
        ("$\\leq$", "≤"),
        ("\\leq", "≤"),
        ("$\\geq$", "≥"),
        ("\\geq", "≥"),
        ("$\\neq$", "≠"),
        ("\\neq", "≠"),
        ("$\\approx$", "≈"),
        ("\\approx", "≈"),
        // math
        ("$\\times$", "×"),
        ("\\times", "×"),
        ("$\\div$", "÷"),
        ("\\div", "÷"),
        ("$\\pm$", "±"),
        ("\\pm", "±"),
        ("$\\infty$", "∞"),
        ("\\infty", "∞"),
        ("$\\cdot$", "·"),
        ("\\cdot", "·"),
        // sets
        ("$\\in$", "∈"),
        ("$\\subset$", "⊂"),
        ("$\\cup$", "∪"),
        ("$\\cap$", "∩"),
    ] {
        s = s.replace(pat, uni);
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_markdown_alerts() {
        let input = "> [!NOTE] This is a note\n> [!TIP] Helpful advice\n> [!WARNING] Be careful";
        let output = format_markdown_bold(input);
        assert!(output.contains("│ NOTE:"));
        assert!(output.contains("│ TIP:"));
        assert!(output.contains("│ WARNING:"));
    }

    #[test]
    fn test_format_markdown_badges() {
        let input = "Here is a [badge:green Success] and [badge:blue Info] tag.";
        let output = format_markdown_bold(input);
        assert!(output.contains("Success"));
        assert!(output.contains("Info"));
    }

    #[test]
    fn test_format_markdown_ui_grid() {
        let json_block = "```ui-grid\n[{\"title\": \"SendChat\", \"desc\": \"Fast video compression\", \"badge\": \"Fast\"}]\n```";
        let output = format_markdown_bold(json_block);
        assert!(output.contains("SendChat"));
        assert!(output.contains("Fast video compression"));
        assert!(output.contains("[◆]"));
    }

    #[test]
    fn test_format_markdown_ui_card() {
        let json_block = "```ui-card\n[{\"title\": \"SubmitKit\", \"subtitle\": \"File packager\", \"badge\": \"Featured\"}]\n```";
        let output = format_markdown_bold(json_block);
        assert!(output.contains("SubmitKit"));
        assert!(output.contains("File packager"));
    }

    #[test]
    fn test_format_markdown_ui_mockup() {
        let json_block = "```ui-mockup\n{\"title\": \"SendSmall\", \"dropzoneText\": \"Drop video here\", \"metrics\": {\"Size\": \"25MB\"}}\n```";
        let output = format_markdown_bold(json_block);
        assert!(output.contains("SendSmall"));
        assert!(output.contains("Drop video here"));
        assert!(output.contains("Size"));
    }

    #[test]
    fn test_format_markdown_single_line_ui_blocks() {
        let single_line_card =
            "ui-card [{\"title\": \"QuickCard\", \"subtitle\": \"Instant preview\"}]";
        let output = format_markdown_bold(single_line_card);
        assert!(output.contains("QuickCard"));
        assert!(output.contains("Instant preview"));

        let fenced_single_line =
            "```ui-grid [{\"title\": \"OptionA\", \"desc\": \"Best choice\"}]```";
        let output_grid = format_markdown_bold(fenced_single_line);
        assert!(output_grid.contains("OptionA"));
        assert!(output_grid.contains("Best choice"));
    }

    #[test]
    fn test_progressive_units_splits_text_and_code_line_by_line_but_groups_tables_and_ui_cards() {
        let doc = "\
# Heading
Paragraph line 1
| Col A | Col B |
| --- | --- |
| 1 | 2 |
After table
```rust
fn main() {}
```
```ui-card
[{\"title\": \"CardTitle\", \"subtitle\": \"Sub\"}]
```
Final line";

        let units = split_progressive_units(doc, true);
        assert_eq!(
            units,
            vec![
                "# Heading",
                "Paragraph line 1",
                "| Col A | Col B |\n| --- | --- |\n| 1 | 2 |",
                "After table",
                "```rust\nfn main() {}",
                "```",
                "```ui-card\n[{\"title\": \"CardTitle\", \"subtitle\": \"Sub\"}]\n```",
                "Final line",
            ]
        );
    }

    #[test]
    fn test_progressive_units_holds_incomplete_table_and_unclosed_ui_card_while_streaming() {
        let streaming_table = "Intro line\n| Header 1 | Header 2 |\n| --- | --- |\n";
        let units = split_progressive_units(streaming_table, false);
        assert_eq!(units, vec!["Intro line"]);

        let streaming_card = "Intro line\n```ui-grid\n[{\"title\": \"A\"}]\n";
        let units_card = split_progressive_units(streaming_card, false);
        assert_eq!(units_card, vec!["Intro line"]);

        // Bare opening fence without its first code line is held so an empty code box never flashes
        let bare_fence = "Intro line\n```rust\n";
        assert_eq!(
            split_progressive_units(bare_fence, false),
            vec!["Intro line"]
        );

        let streaming_code = "Intro line\n```rust\nlet x = 1;\nlet y = 2;\n";
        let units_code = split_progressive_units(streaming_code, false);
        assert_eq!(
            units_code,
            vec!["Intro line", "```rust\nlet x = 1;", "let y = 2;"]
        );

        // Unlabeled fence whose first line is a ui-card keyword is promoted to atomic block
        let unlabeled_card = "Intro line\n```\nui-card [\n  {\"title\": \"A\"}\n]\n";
        assert_eq!(
            split_progressive_units(unlabeled_card, false),
            vec!["Intro line"]
        );
        let closed_unlabeled_card = "Intro line\n```\nui-card [\n  {\"title\": \"A\"}\n]\n```\n";
        assert_eq!(
            split_progressive_units(closed_unlabeled_card, false),
            vec!["Intro line", "```\nui-card [\n  {\"title\": \"A\"}\n]\n```"]
        );
    }

    #[test]
    fn test_progressive_markdown_pacer_reveals_units_progressively_and_flushes() {
        let mut pacer = ProgressiveMarkdownPacer::default();
        pacer.push_delta("Line 1\nLine 2\n| A | B |\n| - | - |\n");
        assert_eq!(pacer.step().as_deref(), Some("Line 1"));
        assert_eq!(pacer.step().as_deref(), Some("Line 1\nLine 2"));
        // Table is still open because no non-table line or finish has arrived
        assert_eq!(pacer.step(), None);
        assert!(!pacer.is_done());

        pacer.finish_with_authoritative("Line 1\nLine 2\n| A | B |\n| - | - |\nDone");
        assert_eq!(
            pacer.step().as_deref(),
            Some("Line 1\nLine 2\n| A | B |\n| - | - |")
        );
        assert_eq!(
            pacer.step().as_deref(),
            Some("Line 1\nLine 2\n| A | B |\n| - | - |\nDone")
        );
        assert!(pacer.is_done());
    }
}

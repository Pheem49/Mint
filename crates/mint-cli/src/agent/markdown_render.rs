use super::*;

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
    let theme = &THEME_SET.themes["base16-ocean.dark"];
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
    let title = val.get("title").and_then(|v| v.as_str()).unwrap_or("OPTIONS");
    let mut lines = Vec::new();
    lines.push(format!("\x1b[38;2;56;189;248m┌─ {title} {}\x1b[0m", "─".repeat(dashes)));

    if let Some(subtitle) = val.get("subtitle").and_then(|v| v.as_str()) {
        lines.push(format!("\x1b[38;2;56;189;248m│\x1b[0m \x1b[90m{subtitle}\x1b[0m"));
        lines.push(format!("\x1b[38;2;56;189;248m│\x1b[0m"));
    }

    for (idx, item) in items.iter().enumerate() {
        let item_title = item.get("title").and_then(|v| v.as_str()).unwrap_or("");
        let item_desc = item.get("desc").and_then(|v| v.as_str()).unwrap_or("");
        let item_badge = item.get("badge").and_then(|v| v.as_str());

        let badge_str = if let Some(b) = item_badge {
            format!(" \x1b[1;38;2;52;211;153m[{b}]\x1b[0m")
        } else {
            String::new()
        };

        lines.push(format!(
            "\x1b[38;2;56;189;248m│\x1b[0m \x1b[38;2;56;189;248m[◆]\x1b[0m \x1b[1m{item_title}\x1b[0m{badge_str}"
        ));
        if !item_desc.is_empty() {
            lines.push(format!("\x1b[38;2;56;189;248m│\x1b[0m     \x1b[90m{item_desc}\x1b[0m"));
        }
        if idx < items.len() - 1 {
            lines.push(format!("\x1b[38;2;56;189;248m│\x1b[0m"));
        }
    }

    lines.push(format!("\x1b[38;2;56;189;248m└{}\x1b[0m", "─".repeat(dashes + 4)));
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
        let title = item.get("title").and_then(|v| v.as_str()).unwrap_or("FEATURE");
        let subtitle = item.get("subtitle").and_then(|v| v.as_str());
        let badge = item.get("badge").and_then(|v| v.as_str());

        let badge_str = if let Some(b) = badge {
            format!(" \x1b[1;38;2;52;211;153m[{b}]\x1b[0m")
        } else {
            String::new()
        };

        lines.push(format!(
            "\x1b[38;2;52;211;153m┌─ \x1b[1m{title}\x1b[0m{badge_str} {}\x1b[0m",
            "─".repeat(dashes.saturating_sub(title.len() + 2))
        ));

        if let Some(sub) = subtitle {
            lines.push(format!("\x1b[38;2;52;211;153m│\x1b[0m {sub}"));
        }

        if let Some(details) = item.get("details").and_then(|v| v.as_object()) {
            for (k, v) in details {
                let val_str = v.as_str().unwrap_or("");
                lines.push(format!(
                    "\x1b[38;2;52;211;153m│\x1b[0m   \x1b[1m{k}:\x1b[0m {val_str}"
                ));
            }
        }

        lines.push(format!("\x1b[38;2;52;211;153m└{}\x1b[0m", "─".repeat(dashes + 4)));
        if idx < items.len() - 1 {
            lines.push(String::new());
        }
    }

    Some(lines)
}

fn render_cli_ui_mockup(json_str: &str, term_width: usize) -> Option<Vec<String>> {
    let val: serde_json::Value = serde_json::from_str(json_str).ok()?;
    let title = val.get("title").and_then(|v| v.as_str()).unwrap_or("Mockup");
    let subtitle = val.get("subtitle").and_then(|v| v.as_str());
    let dropzone_text = val.get("dropzoneText").and_then(|v| v.as_str()).unwrap_or("Drop file here");
    let disclaimer = val.get("disclaimer").and_then(|v| v.as_str()).unwrap_or("UI Concept Only");

    let dashes = term_width.saturating_sub(14).clamp(16, 50);
    let mut lines = Vec::new();

    lines.push(format!(
        "\x1b[38;2;192;132;252m┌─ [MOCKUP] \x1b[1m{title}\x1b[0m {}\x1b[0m",
        "─".repeat(dashes.saturating_sub(title.len() + 10))
    ));

    if let Some(sub) = subtitle {
        lines.push(format!("\x1b[38;2;192;132;252m│\x1b[0m \x1b[90m{sub}\x1b[0m"));
    }

    lines.push(format!("\x1b[38;2;192;132;252m│\x1b[0m  \x1b[90m┌─ Dropzone ────────────────────────┐\x1b[0m"));
    lines.push(format!("\x1b[38;2;192;132;252m│\x1b[0m  \x1b[90m│\x1b[0m \x1b[38;2;56;189;248m[↑]\x1b[0m {dropzone_text}         \x1b[90m│\x1b[0m"));
    lines.push(format!("\x1b[38;2;192;132;252m│\x1b[0m  \x1b[90m└───────────────────────────────────┘\x1b[0m"));

    if let Some(metrics) = val.get("metrics").and_then(|v| v.as_object()) {
        for (k, v) in metrics {
            let val_str = v.as_str().unwrap_or("");
            lines.push(format!("\x1b[38;2;192;132;252m│\x1b[0m   \x1b[90m{k}:\x1b[0m \x1b[1m{val_str}\x1b[0m"));
        }
    }

    lines.push(format!("\x1b[38;2;192;132;252m│\x1b[0m  \x1b[90mℹ {disclaimer}\x1b[0m"));
    lines.push(format!("\x1b[38;2;192;132;252m└{}\x1b[0m", "─".repeat(dashes + 4)));

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
    for prefix in &["ui-card", "ui_card", "ui-card-json", "ui_card_json", "ui-feature", "ui_feature"] {
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

pub(super) fn format_markdown_bold(text: &str) -> String {
    let (term_width, _) = markdown::terminal_size_or_default();
    let term_width = term_width as usize;

    let mut formatted_lines = Vec::new();
    let mut in_code_block = false;
    let mut highlighter: Option<HighlightLines> = None;
    let mut active_custom_block: Option<String> = None;
    let mut custom_block_buffer: Vec<String> = Vec::new();

    for line in text.lines() {
        let mut formatted_line = line.to_string();
        let trimmed = line.trim_start();

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
                        "ui-card" | "ui_card" | "ui-card-json" | "ui_card_json" | "ui-feature" | "ui_feature" => {
                            render_cli_ui_card(&buffered, term_width)
                        }
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
                            formatted_line = format!("{CODE_BORDER}│{RESET} {l}");
                            formatted_lines.push(formatted_line);
                        }
                        formatted_lines.push(code_block_bottom_border(term_width));
                    }
                } else {
                    formatted_lines.push(code_block_bottom_border(term_width));
                }
            } else {
                let lang_raw = trimmed.trim_start_matches('`').trim();
                let (lang, rest) = if let Some(space_idx) = lang_raw.find(|c: char| c.is_whitespace() || c == '[' || c == '{') {
                    (&lang_raw[..space_idx], lang_raw[space_idx..].trim())
                } else {
                    (lang_raw, "")
                };

                match lang {
                    "ui-grid" | "ui_grid" | "ui-grid-json" | "ui_grid_json"
                    | "ui-card" | "ui_card" | "ui-card-json" | "ui_card_json" | "ui-feature" | "ui_feature"
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
            formatted_line = format!("{CODE_BORDER}│{RESET} {content}{RESET}");
            formatted_lines.push(formatted_line);
            continue;
        }

        // Handle blockquotes / GitHub alerts (both with and without leading '>')
        let is_quote = trimmed.starts_with('>');
        let is_standalone_alert = trimmed.starts_with("[!");
        if is_quote || is_standalone_alert {
            let quote_content = if is_quote { trimmed[1..].trim() } else { trimmed };
            let upper = quote_content.to_uppercase();
            if upper.starts_with("[!NOTE]") {
                let body = quote_content[7..].trim();
                formatted_lines.push(format!("\x1b[38;2;56;189;248m│ NOTE:\x1b[0m {}", process_inline_bold(body)));
                continue;
            } else if upper.starts_with("[!TIP]") {
                let body = quote_content[6..].trim();
                formatted_lines.push(format!("\x1b[38;2;52;211;153m│ TIP:\x1b[0m {}", process_inline_bold(body)));
                continue;
            } else if upper.starts_with("[!IMPORTANT]") {
                let body = quote_content[12..].trim();
                formatted_lines.push(format!("\x1b[38;2;192;132;252m│ IMPORTANT:\x1b[0m {}", process_inline_bold(body)));
                continue;
            } else if upper.starts_with("[!WARNING]") {
                let body = quote_content[10..].trim();
                formatted_lines.push(format!("\x1b[38;2;251;191;36m│ WARNING:\x1b[0m {}", process_inline_bold(body)));
                continue;
            } else if upper.starts_with("[!CAUTION]") {
                let body = quote_content[10..].trim();
                formatted_lines.push(format!("\x1b[38;2;248;113;113m│ CAUTION:\x1b[0m {}", process_inline_bold(body)));
                continue;
            } else if is_quote {
                formatted_lines.push(format!("\x1b[38;2;100;116;139m│\x1b[0m \x1b[3m{}\x1b[0m", process_inline_bold(quote_content)));
                continue;
            }
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
            new_line.push('•');
            new_line.push_str(&line[leading_spaces + marker_len..]);
            formatted_line = process_inline_bold(&new_line);
        } else {
            let hash_count = trimmed.chars().take_while(|&c| c == '#').count();
            let is_heading = (1..=6).contains(&hash_count)
                && trimmed.as_bytes().get(hash_count) == Some(&b' ');
            if is_heading {
                let leading_len = line.len() - trimmed.len();
                let leading_spaces_str = &line[..leading_len];
                let heading_text = trimmed[hash_count + 1..].trim_end();
                let (style_start, style_end) = match hash_count {
                    1 => ("\x1b[1m\x1b[4m\x1b[38;2;56;189;248m", RESET),
                    2 => ("\x1b[1m\x1b[38;2;56;189;248m", RESET),
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
                    "blue" => "\x1b[1;38;2;56;189;248m",
                    "purple" => "\x1b[1;38;2;192;132;252m",
                    "amber" | "yellow" => "\x1b[1;38;2;251;191;36m",
                    "red" => "\x1b[1;38;2;248;113;113m",
                    _ => "\x1b[1;38;2;52;211;153m",
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

pub(super) fn process_inline_bold(text: &str) -> String {
    let count = text.matches("**").count();
    let pair_limit = (count / 2) * 2;
    let mut result = String::with_capacity(text.len());
    let parts = text.split("**");
    let mut is_bold = false;
    let mut processed_markers = 0;
    for part in parts {
        if is_bold && processed_markers < pair_limit {
            result.push_str(BLUE);
            result.push_str(part);
            result.push_str(RESET);
        } else {
            result.push_str(part);
        }
        processed_markers += 1;
        is_bold = !is_bold;
    }
    process_inline_badges(&result)
}

/// Replace common LaTeX math symbols with Unicode equivalents.
/// Fixes garbled output like "ightarrow$" from models that emit LaTeX notation.
pub(super) fn sanitize_latex(text: &str) -> String {
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
        let single_line_card = "ui-card [{\"title\": \"QuickCard\", \"subtitle\": \"Instant preview\"}]";
        let output = format_markdown_bold(single_line_card);
        assert!(output.contains("QuickCard"));
        assert!(output.contains("Instant preview"));

        let fenced_single_line = "```ui-grid [{\"title\": \"OptionA\", \"desc\": \"Best choice\"}]```";
        let output_grid = format_markdown_bold(fenced_single_line);
        assert!(output_grid.contains("OptionA"));
        assert!(output_grid.contains("Best choice"));
    }
}


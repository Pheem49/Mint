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

pub(super) fn format_markdown_bold(text: &str) -> String {
    let (term_width, _) = markdown::terminal_size_or_default();
    let term_width = term_width as usize;

    let mut formatted_lines = Vec::new();
    let mut in_code_block = false;
    let mut highlighter: Option<HighlightLines> = None;

    for line in text.lines() {
        let mut formatted_line = line.to_string();
        let trimmed = line.trim_start();

        if trimmed.starts_with("```") {
            let was_in_code_block = in_code_block;
            in_code_block = !in_code_block;
            if was_in_code_block {
                highlighter = None;
                formatted_lines.push(code_block_bottom_border(term_width));
            } else {
                let lang = trimmed.trim_start_matches('`').trim();
                formatted_lines.push(code_block_top_border(lang, term_width));
                highlighter = start_code_highlighter(lang);
            }
            continue;
        }

        if !in_code_block {
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
                // '#' and ' ' are both single-byte ASCII, so these are
                // always valid char-boundary slice points.
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
        } else {
            // Do not reformat markdown inside code blocks — only add a left
            // gutter bar so the block reads as visually distinct from
            // prose, matching the border drawn around it. The content itself
            // is syntax-highlighted when the fence's language hint matched a
            // known syntax; otherwise it's printed as-is, same as before.
            let content = match highlighter.as_mut() {
                Some(h) => highlight_code_line(h, line),
                None => line.to_string(),
            };
            formatted_line = format!("{CODE_BORDER}│{RESET} {content}{RESET}");
        }

        formatted_lines.push(formatted_line);
    }

    let mut result = formatted_lines.join("\n");
    if text.ends_with('\n') {
        result.push('\n');
    }
    result
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
    result
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

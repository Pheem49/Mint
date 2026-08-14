/// Terminal (cols, rows), defaulting to 80x24 both when the query errors
/// *and* when it succeeds but reports a degenerate 0-sized dimension — the
/// latter has been observed for real (not just theoretically) when running
/// under certain pty layers with no attached terminal emulator to answer the
/// underlying ioctl meaningfully. `unwrap_or` alone only catches the `Err`
/// case, not an `Ok((0, 0))`, so every caller that used
/// `crossterm::terminal::size().unwrap_or((80, 24))` directly was exposed to
/// wrapping every line down to one character when that happened.
pub fn terminal_size_or_default() -> (u16, u16) {
    let (cols, rows) = crossterm::terminal::size().unwrap_or((80, 24));
    (if cols == 0 { 80 } else { cols }, if rows == 0 { 24 } else { rows })
}

pub fn is_table_line(line: &str) -> bool {
    let trimmed = line.trim();
    trimmed.starts_with('|') && trimmed.ends_with('|') && trimmed.len() > 1
}

pub fn unicode_width(s: &str) -> usize {
    let mut count = 0;
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\u{1b}' && chars.peek() == Some(&'[') {
            chars.next();
            for c2 in chars.by_ref() {
                if c2.is_ascii_alphabetic() {
                    break;
                }
            }
            continue;
        }
        let val = c as u32;
        if (0x0E31..=0x0E31).contains(&val)
            || (0x0E34..=0x0E3A).contains(&val)
            || (0x0E47..=0x0E4E).contains(&val)
        {
            continue;
        }
        if is_cjk(c) {
            count += 2;
        } else {
            count += 1;
        }
    }
    count
}

pub fn is_cjk(c: char) -> bool {
    let val = c as u32;
    (0x4E00..=0x9FFF).contains(&val)
        || (0x3400..=0x4DBF).contains(&val)
        || (0x20000..=0x2A6DF).contains(&val)
        || (0xAC00..=0xD7AF).contains(&val)
        || (0x3040..=0x309F).contains(&val)
        || (0x30A0..=0x30FF).contains(&val)
}

/// Splits `text` into lines that each fit within `max_width` (measured with
/// `unicode_width`, so Thai combining marks/CJK are handled the same way
/// column widths already are). Breaks at whitespace when possible; a single
/// "word" longer than `max_width` (a URL, an unbroken code token, ...) is
/// hard-split at the width limit rather than left overflowing, since an
/// overflowing line is exactly the broken-border bug this exists to avoid.
/// Returns `vec![String::new()]` for empty input so callers always get at
/// least one line to size/pad against.
fn wrap_cell_text(text: &str, max_width: usize) -> Vec<String> {
    if max_width == 0 || text.is_empty() {
        return vec![text.to_string()];
    }
    let mut lines = Vec::new();
    for source_line in text.split('\n') {
        let mut current = String::new();
        let mut current_width = 0;
        for word in source_line.split(' ') {
            if word.is_empty() {
                continue;
            }
            let word_width = unicode_width(word);
            let sep_width = if current.is_empty() { 0 } else { 1 };
            if current_width + sep_width + word_width <= max_width {
                if !current.is_empty() {
                    current.push(' ');
                    current_width += 1;
                }
                current.push_str(word);
                current_width += word_width;
                continue;
            }
            if !current.is_empty() {
                lines.push(std::mem::take(&mut current));
                current_width = 0;
            }
            if word_width <= max_width {
                current.push_str(word);
                current_width = word_width;
            } else {
                // Single word wider than the column: hard-split character by
                // character so it still fits, rather than overflowing.
                for c in word.chars() {
                    let c_width = unicode_width(&c.to_string());
                    if current_width + c_width > max_width && !current.is_empty() {
                        lines.push(std::mem::take(&mut current));
                        current_width = 0;
                    }
                    current.push(c);
                    current_width += c_width;
                }
            }
        }
        lines.push(current);
    }
    if lines.is_empty() {
        lines.push(String::new());
    }
    lines
}

/// Hard-wraps `text` to `max_width` visual columns, breaking wherever the
/// width limit is hit rather than at word boundaries, and preserving every
/// character exactly — including runs of whitespace. Unlike `wrap_cell_text`
/// (which collapses whitespace between words when it re-flows text), this is
/// for content where whitespace is semantically significant, such as
/// code/diff lines where collapsing leading indentation would misrepresent
/// the change being previewed.
pub(crate) fn hard_wrap(text: &str, max_width: usize) -> Vec<String> {
    if max_width == 0 || text.is_empty() {
        return vec![text.to_string()];
    }
    let mut lines = Vec::new();
    let mut current = String::new();
    let mut current_width = 0;
    for c in text.chars() {
        let c_width = unicode_width(&c.to_string());
        if current_width + c_width > max_width && !current.is_empty() {
            lines.push(std::mem::take(&mut current));
            current_width = 0;
        }
        current.push(c);
        current_width += c_width;
    }
    lines.push(current);
    lines
}

pub fn render_markdown_table(table_lines: &[String]) -> String {
    let mut rows: Vec<Vec<String>> = Vec::new();
    for line in table_lines {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let content = &trimmed[1..trimmed.len() - 1];
        let content_escaped = content.replace("\\|", "\u{0000}");
        let cols: Vec<String> = content_escaped
            .split('|')
            .map(|s| s.replace("\u{0000}", "|").trim().to_string())
            .collect();
        rows.push(cols);
    }

    if rows.is_empty() {
        return String::new();
    }

    let mut has_separator = false;
    let mut separator_idx = None;
    for (i, row) in rows.iter().enumerate() {
        if !row.is_empty()
            && row.iter().all(|col| {
                col.chars()
                    .all(|c| c == '-' || c == ':' || c == ' ' || c == '\t')
            })
            && row.iter().any(|col| col.contains('-'))
        {
            has_separator = true;
            separator_idx = Some(i);
            break;
        }
    }

    let mut data_rows: Vec<Vec<String>> = Vec::new();
    let mut header_row: Option<Vec<String>> = None;

    if has_separator {
        let sep_idx = separator_idx.unwrap();
        if sep_idx > 0 {
            header_row = Some(rows[sep_idx - 1].clone());
            for (i, row) in rows.iter().enumerate() {
                if i != sep_idx && i != sep_idx - 1 {
                    data_rows.push(row.clone());
                }
            }
        } else {
            for (i, row) in rows.iter().enumerate() {
                if i != sep_idx {
                    data_rows.push(row.clone());
                }
            }
        }
    } else {
        header_row = Some(rows[0].clone());
        data_rows = rows[1..].to_vec();
    }

    let num_cols = rows.iter().map(|r| r.len()).max().unwrap_or(0);
    let mut col_widths = vec![0; num_cols];

    if let Some(ref header) = header_row {
        for (i, col) in header.iter().enumerate() {
            if i < num_cols {
                col_widths[i] = col_widths[i].max(unicode_width(col));
            }
        }
    }
    for row in &data_rows {
        for (i, col) in row.iter().enumerate() {
            if i < num_cols {
                col_widths[i] = col_widths[i].max(unicode_width(col));
            }
        }
    }

    // Shrink columns to fit the terminal when their natural (unwrapped) width
    // would overflow it — proportional to each column's natural share of the
    // space, with a floor so no column collapses to unreadable. Cell content
    // that no longer fits gets word-wrapped below rather than left to overflow
    // and break the border characters (see `wrap_cell_text`).
    if num_cols > 0 {
        let (term_width, _) = terminal_size_or_default();
        // Each column contributes `width + 2` (one space of padding either
        // side) plus one border character; there's one extra border character
        // closing the row.
        let overhead = 3 * num_cols + 1;
        let available = (term_width as usize).saturating_sub(overhead);
        let natural_total: usize = col_widths.iter().sum();
        if natural_total > available && available > 0 {
            const MIN_COL_WIDTH: usize = 8;
            let floor_total = MIN_COL_WIDTH * num_cols;
            if available <= floor_total {
                col_widths = vec![(available / num_cols).max(1); num_cols];
            } else {
                let flexible = available - floor_total;
                for width in col_widths.iter_mut() {
                    let share = (*width as f64 / natural_total as f64 * flexible as f64) as usize;
                    *width = MIN_COL_WIDTH + share;
                }
            }
        }
    }

    let mut rendered = String::new();

    let draw_border =
        |left: &str, mid: &str, right: &str, fill: &str, widths: &[usize]| -> String {
            let mut s = left.to_string();
            for (i, &w) in widths.iter().enumerate() {
                s.push_str(&fill.repeat(w + 2));
                if i < widths.len() - 1 {
                    s.push_str(mid);
                }
            }
            s.push_str(right);
            s.push('\n');
            s
        };

    // Wraps every cell in `row` to its column's width and returns the row as
    // `Vec<physical line>`, each already a complete `│ ... │ ... │` string —
    // shorter cells are blank-padded so every column has the same number of
    // physical lines. `style` optionally wraps each cell's wrapped lines
    // (used for the header's bold/cyan color).
    let render_wrapped_row =
        |row: &[String], widths: &[usize], style: Option<(&str, &str)>| -> String {
            let per_col: Vec<Vec<String>> = (0..num_cols)
                .map(|i| {
                    let col_val = row.get(i).cloned().unwrap_or_default();
                    wrap_cell_text(&col_val, widths[i])
                })
                .collect();
            let line_count = per_col.iter().map(|c| c.len()).max().unwrap_or(1);
            let mut out = String::new();
            for line_idx in 0..line_count {
                out.push('│');
                for i in 0..num_cols {
                    let empty = String::new();
                    let text = per_col[i].get(line_idx).unwrap_or(&empty);
                    let width = unicode_width(text);
                    let padding = widths[i].saturating_sub(width);
                    match style {
                        Some((prefix, suffix)) => out.push_str(&format!(
                            " {prefix}{}{suffix}{}",
                            text,
                            " ".repeat(padding + 1)
                        )),
                        None => out.push_str(&format!(" {}{}", text, " ".repeat(padding + 1))),
                    }
                    out.push('│');
                }
                out.push('\n');
            }
            out
        };

    rendered.push_str(&draw_border("┌", "┬", "┐", "─", &col_widths));

    if let Some(ref header) = header_row {
        rendered.push_str(&render_wrapped_row(
            header,
            &col_widths,
            Some(("\x1b[1;36m", "\x1b[0m")),
        ));
        rendered.push_str(&draw_border("├", "┼", "┤", "─", &col_widths));
    }

    for (r_idx, row) in data_rows.iter().enumerate() {
        rendered.push_str(&render_wrapped_row(row, &col_widths, None));

        if r_idx < data_rows.len() - 1 {
            rendered.push_str(&draw_border("├", "┼", "┤", "─", &col_widths));
        }
    }

    rendered.push_str(draw_border("└", "┴", "┘", "─", &col_widths).trim_end());
    rendered
}

#[cfg(test)]
mod table_tests {
    use super::*;

    fn lines(rows: &[&str]) -> Vec<String> {
        rows.iter().map(|s| s.to_string()).collect()
    }

    /// Every physical line of a rendered table must have identical display
    /// width — otherwise the border characters don't line up in the terminal.
    fn assert_borders_aligned(rendered: &str) {
        let widths: Vec<usize> = rendered
            .lines()
            .filter(|l| !l.is_empty())
            .map(unicode_width)
            .collect();
        assert!(!widths.is_empty(), "table rendered no lines");
        let first = widths[0];
        for (i, w) in widths.iter().enumerate() {
            assert_eq!(*w, first, "line {i} width {w} != {first} in:\n{rendered}");
        }
    }

    #[test]
    fn short_table_renders_on_single_lines_per_row() {
        let table = lines(&["| a | b |", "| --- | --- |", "| 1 | 2 |"]);
        let rendered = render_markdown_table(&table);
        assert_borders_aligned(&rendered);
        // No wrapping needed, so exactly 4 physical lines (top border,
        // header, separator, one data row) plus the closing border.
        assert_eq!(rendered.lines().count(), 5);
    }

    #[test]
    fn long_cell_content_wraps_instead_of_overflowing_terminal_width() {
        let long_cell = "word ".repeat(60); // far wider than any terminal
        let table = lines(&[
            "| name | description |",
            "| --- | --- |",
            &format!("| x | {long_cell} |"),
        ]);
        let rendered = render_markdown_table(&table);
        assert_borders_aligned(&rendered);
        // Falls back to 80 columns when no real terminal is attached (test
        // harness has no TTY), so every physical line must fit within that.
        for line in rendered.lines() {
            assert!(
                unicode_width(line) <= 80,
                "line exceeds terminal width: {line}"
            );
        }
        // The long cell must have actually wrapped across multiple physical
        // lines rather than being truncated or left on one long line.
        assert!(rendered.lines().count() > 5);
    }

    #[test]
    fn wrapped_row_keeps_all_columns_present_on_every_physical_line() {
        let table = lines(&[
            "| short | long |",
            "| --- | --- |",
            &format!("| hi | {} |", "verylongsingleword".repeat(10)),
        ]);
        let rendered = render_markdown_table(&table);
        assert_borders_aligned(&rendered);
        for line in rendered.lines().filter(|l| l.starts_with('│')) {
            assert_eq!(
                line.matches('│').count(),
                3,
                "row line missing a column: {line}"
            );
        }
    }

    #[test]
    fn hard_wrap_preserves_leading_indentation_on_every_wrapped_line() {
        // wrap_cell_text (word-based) would collapse these leading spaces —
        // hard_wrap exists specifically so code/diff indentation survives.
        let code = "        if very_long_condition_name and another_condition_flag:";
        let wrapped = hard_wrap(code, 20);
        assert!(
            wrapped[0].starts_with("        "),
            "lost leading indentation: {:?}",
            wrapped[0]
        );
        // Every char from the original must reappear somewhere, in order,
        // with nothing dropped or re-flowed.
        assert_eq!(wrapped.concat(), code);
    }

    #[test]
    fn hard_wrap_breaks_at_exactly_max_width_mid_word() {
        let text = "0123456789ABCDE";
        let wrapped = hard_wrap(text, 10);
        assert_eq!(wrapped, vec!["0123456789".to_string(), "ABCDE".to_string()]);
    }

    #[test]
    fn hard_wrap_short_text_returns_a_single_line() {
        let wrapped = hard_wrap("short", 80);
        assert_eq!(wrapped, vec!["short".to_string()]);
    }

    #[test]
    fn hard_wrap_empty_text_returns_one_empty_line() {
        assert_eq!(hard_wrap("", 20), vec![String::new()]);
    }
}

use super::*;

pub(super) fn diff_stats(diff: &str) -> (usize, usize) {
    let mut additions = 0;
    let mut deletions = 0;
    for line in diff.lines() {
        if line.starts_with("+++ ") || line.starts_with("--- ") {
            continue;
        }
        if line.starts_with('+') {
            additions += 1;
        } else if line.starts_with('-') {
            deletions += 1;
        }
    }
    (additions, deletions)
}

pub(super) fn parse_hunk_header(line: &str) -> Option<(usize, usize)> {
    let trimmed = line.trim();
    if !trimmed.starts_with("@@") {
        return None;
    }
    let parts: Vec<&str> = trimmed.split("@@").collect();
    if parts.len() < 3 {
        return None;
    }
    let header_body = parts[1].trim();
    let mut old_start = 1;
    let mut new_start = 1;

    for token in header_body.split_whitespace() {
        if let Some(num_str) = token.strip_prefix('-') {
            let start_str = num_str.split(',').next().unwrap_or(num_str);
            old_start = start_str.parse().unwrap_or(1);
        } else if let Some(num_str) = token.strip_prefix('+') {
            let start_str = num_str.split(',').next().unwrap_or(num_str);
            new_start = start_str.parse().unwrap_or(1);
        }
    }
    Some((old_start, new_start))
}

/// Compact single-line approval header, e.g. `Update(src/foo.rs) — +3 -1 lines`.
pub(super) fn print_diff_header(action: &str, path: &str, additions: usize, deletions: usize) {
    println!();
    println!(
        "  {BRIGHT}{action}{RESET}({BLUE}{path}{RESET}) {DIM}—{RESET} {GREEN}+{additions}{RESET} {RED}-{deletions}{RESET} {DIM}lines{RESET}"
    );
}

/// Prints one diff row as a full-width color band (like a diff viewer's gutter
/// highlight), padding `content` with spaces out to the terminal width so the
/// background color fills the row instead of just wrapping the text.
pub(super) fn print_diff_band(bg: &str, line_num: usize, content: &str, term_width: usize) {
    let line_num_str = format!("{:>5}", line_num);
    let prefix_visible_len = 2 + line_num_str.chars().count() + 1;
    // Use the app's East-Asian/Thai-combining-mark-aware width, not a raw
    // char count — a naive count over/under-shoots the true terminal column
    // width for wide glyphs and zero-width Thai marks, leaving the band short
    // of (or past) the terminal edge.
    let available_width = term_width.saturating_sub(prefix_visible_len).max(1);
    let blank_line_num = " ".repeat(line_num_str.chars().count());

    // A code/diff line longer than the terminal is hard-wrapped (never
    // word-wrapped, which would collapse indentation) into multiple physical
    // lines so the colored band never overflows past the terminal edge — the
    // same class of bug the markdown table renderer had.
    for (i, wrapped_line) in markdown::hard_wrap(content, available_width)
        .into_iter()
        .enumerate()
    {
        let line_label = if i == 0 {
            &line_num_str
        } else {
            &blank_line_num
        };
        let content_len = crate::interactive::string_visual_width(&wrapped_line);
        let pad_len = available_width.saturating_sub(content_len);
        println!(
            "  {DIM}{line_label}{RESET} {bg}{wrapped_line}{}{RESET}",
            " ".repeat(pad_len)
        );
    }
}

pub(super) fn print_colored_diff(diff: &str) {
    let (term_width, _) = markdown::terminal_size_or_default();
    let term_width = term_width as usize;
    let mut current_old_line = 1;
    let mut current_new_line = 1;

    for line in diff.lines() {
        if line.starts_with("@@") {
            // Still track line numbers from the hunk header, just don't print
            // the raw unified-diff header/marker lines — they're noise here.
            if let Some((old_s, new_s)) = parse_hunk_header(line) {
                current_old_line = old_s;
                current_new_line = new_s;
            }
        } else if line.starts_with("--- ") || line.starts_with("+++ ") {
            // skip: raw diff file-header lines aren't useful in an approval prompt
        } else if let Some(content) = line.strip_prefix('-') {
            print_diff_band(BG_DEL, current_old_line, content, term_width);
            current_old_line += 1;
        } else if let Some(content) = line.strip_prefix('+') {
            print_diff_band(BG_ADD, current_new_line, content, term_width);
            current_new_line += 1;
        } else {
            let line_num_str = format!("{:>5}", current_new_line);
            current_old_line += 1;
            current_new_line += 1;
            println!("  {DIM}{line_num_str}{RESET} {line}");
        }
    }
}

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

pub(crate) fn format_colored_diff_lines(diff: &str, term_width: usize) -> Vec<String> {
    let term_width = term_width.max(20);
    let mut out = Vec::new();
    let mut current_old_line = 1;
    let mut current_new_line = 1;

    let push_band = |out: &mut Vec<String>, bg: &str, line_num: usize, content: &str| {
        let line_num_str = format!("{:>5}", line_num);
        let prefix_visible_len = 2 + line_num_str.chars().count() + 1;
        let available_width = term_width.saturating_sub(prefix_visible_len).max(1);
        let blank_line_num = " ".repeat(line_num_str.chars().count());

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
            out.push(format!(
                "  {DIM}{line_label}{RESET} {bg}{wrapped_line}{}{RESET}",
                " ".repeat(pad_len)
            ));
        }
    };

    for line in diff.lines() {
        if line.starts_with("@@") {
            if let Some((old_s, new_s)) = parse_hunk_header(line) {
                current_old_line = old_s;
                current_new_line = new_s;
            }
        } else if line.starts_with("--- ") || line.starts_with("+++ ") {
            // skip raw diff file-header lines
        } else if let Some(content) = line.strip_prefix('-') {
            push_band(&mut out, BG_DEL, current_old_line, content);
            current_old_line += 1;
        } else if let Some(content) = line.strip_prefix('+') {
            push_band(&mut out, BG_ADD, current_new_line, content);
            current_new_line += 1;
        } else {
            let line_num_str = format!("{:>5}", current_new_line);
            current_old_line += 1;
            current_new_line += 1;
            out.push(format!("  {DIM}{line_num_str}{RESET} {line}"));
        }
    }
    out
}

#[allow(dead_code)]
pub(crate) fn render_diff_to_ratatui_lines(
    diff: &str,
    term_width: usize,
) -> Vec<ratatui::text::Line<'static>> {
    let ansi_joined = format_colored_diff_lines(diff, term_width).join("\n");
    if let Ok(parsed) = ansi_joined.into_text() {
        parsed.lines
    } else {
        diff.lines()
            .map(|l| ratatui::text::Line::raw(l.to_string()))
            .collect()
    }
}

pub(super) fn print_colored_diff(diff: &str) {
    let (term_width, _) = markdown::terminal_size_or_default();
    for line in format_colored_diff_lines(diff, term_width as usize) {
        println!("{line}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn render_diff_to_ratatui_lines_formats_hunks_at_requested_width() {
        let diff = "@@ -1,2 +1,2 @@\n-old line\n+new line\n context";
        let lines = render_diff_to_ratatui_lines(diff, 60);
        assert_eq!(lines.len(), 3);
    }
}

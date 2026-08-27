//! Tiny Markdown builders for slash-command output. The engine returns Markdown
//! strings (user decision): the Web/Desktop UI renders them as a system message,
//! and the CLI runs them through `crate::markdown` before printing.

/// A GitHub-flavored Markdown table. `rows` cells are used verbatim (already
/// stringified / escaped by the caller).
pub fn md_table(headers: &[&str], rows: &[Vec<String>]) -> String {
    let mut out = String::new();
    out.push_str("| ");
    out.push_str(&headers.join(" | "));
    out.push_str(" |\n|");
    for _ in headers {
        out.push_str(" --- |");
    }
    out.push('\n');
    for row in rows {
        out.push_str("| ");
        out.push_str(&row.join(" | "));
        out.push_str(" |\n");
    }
    out
}

/// `### {title}` followed by a blank line.
pub fn md_heading(title: &str) -> String {
    format!("### {title}\n\n")
}

/// Render a stored UTC RFC3339 timestamp (e.g. a cron job's `next_run`) in the
/// local system timezone. Falls back to the raw string if it doesn't parse.
pub fn local_time(rfc3339: &str) -> String {
    match chrono::DateTime::parse_from_rfc3339(rfc3339) {
        Ok(t) => t
            .with_timezone(&chrono::Local)
            .format("%Y-%m-%d %H:%M %:z")
            .to_string(),
        Err(_) => rfc3339.to_string(),
    }
}

/// `- {item}` lines.
pub fn md_list(items: &[String]) -> String {
    items
        .iter()
        .map(|i| format!("- {i}"))
        .collect::<Vec<_>>()
        .join("\n")
}

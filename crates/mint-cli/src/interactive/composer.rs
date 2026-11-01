use std::ops::Range;
use std::path::Path;
use unicode_width::UnicodeWidthStr;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CollapsedPaste {
    pub range: Range<usize>,
    pub text: String,
}

pub(crate) fn normalize_prompt_newlines(text: &str) -> String {
    text.replace("\r\n", "\n").replace('\r', "\n")
}

pub(crate) fn input_row_start(chars: &[char], rows: &[String], row_index: usize) -> usize {
    let mut start = 0;
    for row in rows.iter().take(row_index) {
        start += row.chars().count();
        if chars.get(start) == Some(&'\n') {
            start += 1;
        }
    }
    start
}

pub(crate) fn wrap_input_visual_into_rows(
    input_chars: &[char],
    row_width: usize,
    cursor_pos: usize,
) -> (Vec<String>, usize, usize) {
    let row_width = row_width.max(1);
    let mut rows: Vec<String> = Vec::new();
    let mut row_starts = Vec::new();
    let mut row_start = 0usize;
    let mut visual_width = 0usize;

    for (index, character) in input_chars.iter().copied().enumerate() {
        if character == '\n' {
            rows.push(input_chars[row_start..index].iter().collect());
            row_starts.push(row_start);
            row_start = index + 1;
            visual_width = 0;
            continue;
        }

        let character_width = UnicodeWidthStr::width(character.to_string().as_str());
        if visual_width > 0 && visual_width + character_width > row_width {
            rows.push(input_chars[row_start..index].iter().collect());
            row_starts.push(row_start);
            row_start = index;
            visual_width = 0;
        }
        visual_width += character_width;
    }

    if row_start < input_chars.len() || input_chars.last() == Some(&'\n') || rows.is_empty() {
        rows.push(input_chars[row_start..].iter().collect());
        row_starts.push(row_start);
    }

    let cursor_pos = cursor_pos.min(input_chars.len());
    let mut cursor_row = rows.len().saturating_sub(1);
    for (index, start) in row_starts.iter().copied().enumerate() {
        let end = start + rows[index].chars().count();
        let next_starts_here = row_starts.get(index + 1) == Some(&cursor_pos);
        if cursor_pos < end || (cursor_pos == end && !next_starts_here) {
            cursor_row = index;
            break;
        }
        if cursor_pos == start {
            cursor_row = index;
            break;
        }
    }
    let cursor_col = cursor_pos.saturating_sub(row_starts[cursor_row]);
    (rows, cursor_row, cursor_col)
}

/// Shared `/`, `$`, and `@` completion matcher used across Full-Screen TUI and Classic CLI.
pub(crate) fn match_composer_suggestions(
    chars: &[char],
    cursor: usize,
    current_dir: &Path,
) -> Vec<(String, String)> {
    let cursor = cursor.min(chars.len());
    let before: String = chars[..cursor].iter().collect();
    if before.starts_with('/') && !before.contains(char::is_whitespace) {
        let prefix = before.to_lowercase();
        let query = &prefix[1..];
        let mut matches: Vec<_> = super::SLASH_COMMANDS
            .iter()
            .filter(|command| {
                command.on_cli()
                    && command.token.starts_with('/')
                    && (command.token.to_lowercase().starts_with(&prefix)
                        || command.description.to_lowercase().contains(query))
            })
            .collect();
        matches.sort_by_key(|command| !command.token.to_lowercase().starts_with(&prefix));
        return matches
            .into_iter()
            .map(|command| (command.token.clone(), command.description.clone()))
            .collect();
    }
    if before.starts_with('$') && !before.contains(char::is_whitespace) {
        let prefix = before[1..].to_lowercase();
        return super::load_all_available_skills(current_dir)
            .into_iter()
            .filter(|skill| skill.name.to_lowercase().starts_with(&prefix))
            .map(|skill| {
                (
                    format!("${}", skill.name),
                    skill.description.unwrap_or_default(),
                )
            })
            .collect();
    }
    let start = chars[..cursor]
        .iter()
        .rposition(|c| c.is_whitespace())
        .map_or(0, |index| index + 1);
    if chars.get(start) == Some(&'@') {
        let query: String = chars[start..cursor].iter().collect();
        return super::collect_mention_candidates(&query, current_dir)
            .into_iter()
            .map(|candidate| (candidate.label, candidate.description))
            .collect();
    }
    Vec::new()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wrap_input_visual_handles_newlines_and_thai_marks() {
        let chars: Vec<char> = "สวัสดี\nโลก".chars().collect();
        let (rows, row, col) = wrap_input_visual_into_rows(&chars, 20, chars.len());
        assert_eq!(rows, vec!["สวัสดี".to_string(), "โลก".to_string()]);
        assert_eq!(row, 1);
        assert_eq!(col, 3);
    }

    #[test]
    fn match_composer_suggestions_matches_slash_commands() {
        let chars: Vec<char> = "/res".chars().collect();
        let results = match_composer_suggestions(&chars, chars.len(), Path::new("."));
        assert!(results.iter().any(|(token, _)| token == "/resume"));
    }
}

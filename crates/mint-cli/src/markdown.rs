pub fn is_table_line(line: &str) -> bool {
    let trimmed = line.trim();
    trimmed.starts_with('|') && trimmed.ends_with('|') && trimmed.len() > 1
}

pub fn process_inline_bold(s: &str) -> String {
    let mut result = String::new();
    let parts = s.split("**");
    let mut is_bold = false;
    for part in parts {
        if is_bold {
            result.push_str("\x1b[1m");
            result.push_str(part);
            result.push_str("\x1b[0m");
        } else {
            result.push_str(part);
        }
        is_bold = !is_bold;
    }
    result
}

pub fn format_line(line: &str) -> String {
    let mut formatted = line.to_string();
    let trimmed = line.trim_start();

    // Markdown Images: ![alt](url) → render as  🖼  alt — url
    if let Some(rest) = trimmed.strip_prefix("![") {
        if let Some(bracket_end) = rest.find("](") {
            let alt = &rest[..bracket_end];
            let after = &rest[bracket_end + 2..];
            if let Some(paren_end) = after.find(')') {
                let url = &after[..paren_end];
                return if alt.is_empty() {
                    format!("  🖼  \x1b[36m{url}\x1b[0m")
                } else {
                    format!("  🖼  {alt}\x1b[2m — \x1b[0m\x1b[36m{url}\x1b[0m")
                };
            }
        }
    }

    if (trimmed.starts_with("- ") || trimmed.starts_with("* ") || trimmed.starts_with("+ "))
        && trimmed.len() >= 2
    {
        let leading_len = line.len() - trimmed.len();
        let leading_spaces = &line[..leading_len];
        formatted = format!("{}•{}", leading_spaces, &trimmed[1..]);
    }
    process_inline_bold(&formatted)
}

pub fn unicode_width(s: &str) -> usize {
    let mut count = 0;
    for c in s.chars() {
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
        if row.iter().all(|col| {
            col.chars()
                .all(|c| c == '-' || c == ':' || c == ' ' || c == '\t')
        }) && !row.is_empty()
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

    rendered.push_str(&draw_border("┌", "┬", "┐", "─", &col_widths));

    if let Some(ref header) = header_row {
        rendered.push('│');
        for (i, col) in header.iter().enumerate() {
            let width = unicode_width(col);
            let padding = col_widths[i] - width;
            rendered.push_str(&format!(
                " \x1b[1;36m{}\x1b[0m{}",
                col,
                " ".repeat(padding + 1)
            ));
            rendered.push('│');
        }
        rendered.push('\n');
        rendered.push_str(&draw_border("├", "┼", "┤", "─", &col_widths));
    }

    for (r_idx, row) in data_rows.iter().enumerate() {
        rendered.push('│');
        for i in 0..num_cols {
            let col_val = row.get(i).cloned().unwrap_or_default();
            let width = unicode_width(&col_val);
            let padding = col_widths[i] - width;
            rendered.push_str(&format!(" {}{}", col_val, " ".repeat(padding + 1)));
            rendered.push('│');
        }
        rendered.push('\n');

        if r_idx < data_rows.len() - 1 {
            rendered.push_str(&draw_border("├", "┼", "┤", "─", &col_widths));
        }
    }

    rendered.push_str(draw_border("└", "┴", "┘", "─", &col_widths).trim_end());
    rendered
}

pub struct CliStreamFormatter {
    line_buffer: String,
    table_buffer: Vec<String>,
    in_code_block: bool,
}

impl CliStreamFormatter {
    pub fn new() -> Self {
        Self {
            line_buffer: String::new(),
            table_buffer: Vec::new(),
            in_code_block: false,
        }
    }

    pub fn process_char(&mut self, c: char, mut print_fn: impl FnMut(char)) {
        self.line_buffer.push(c);
        if c == '\n' {
            let mut line = self.line_buffer.clone();
            self.line_buffer.clear();
            if line.ends_with('\n') {
                line.pop();
            }

            let trimmed = line.trim();
            if trimmed.starts_with("```") {
                self.in_code_block = !self.in_code_block;
            }

            if self.in_code_block {
                if !self.table_buffer.is_empty() {
                    self.flush_table(&mut print_fn);
                }
                for ch in line.chars() {
                    print_fn(ch);
                }
                print_fn('\n');
                for ch in "  ".chars() {
                    print_fn(ch);
                }
            } else {
                if is_table_line(&line) {
                    self.table_buffer.push(line);
                } else {
                    if !self.table_buffer.is_empty() {
                        self.flush_table(&mut print_fn);
                    }
                    let formatted = format_line(&line);
                    for ch in formatted.chars() {
                        print_fn(ch);
                    }
                    print_fn('\n');
                    for ch in "  ".chars() {
                        print_fn(ch);
                    }
                }
            }
        }
    }

    pub fn finalize(&mut self, mut print_fn: impl FnMut(char)) {
        if !self.line_buffer.is_empty() {
            let mut line = self.line_buffer.clone();
            self.line_buffer.clear();
            if line.ends_with('\n') {
                line.pop();
            }
            if self.in_code_block {
                if !self.table_buffer.is_empty() {
                    self.flush_table(&mut print_fn);
                }
                for ch in line.chars() {
                    print_fn(ch);
                }
            } else if is_table_line(&line) {
                self.table_buffer.push(line);
            } else {
                if !self.table_buffer.is_empty() {
                    self.flush_table(&mut print_fn);
                }
                let formatted = format_line(&line);
                for ch in formatted.chars() {
                    print_fn(ch);
                }
            }
        }
        if !self.table_buffer.is_empty() {
            self.flush_table(&mut print_fn);
        }
    }

    fn flush_table<F: FnMut(char)>(&mut self, mut print_fn: F) {
        let rendered = render_markdown_table(&self.table_buffer);
        for ch in rendered.chars() {
            print_fn(ch);
        }
        print_fn('\n');
        self.table_buffer.clear();
    }
}

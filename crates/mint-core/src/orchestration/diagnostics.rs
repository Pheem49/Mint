//! Structured Self-Correction Diagnostics Engine
//!
//! Analyzes tool execution failures (such as patch text mismatches, ambiguous replacement targets,
//! and compiler/runtime execution errors) and produces actionable self-correction guidance
//! for the autonomous agent.

use std::path::Path;

/// Formats an error with actionable self-correction advice.
pub fn format_patch_not_found_guidance(
    file_content: &str,
    old_text: &str,
    hunk_index: usize,
    path: &Path,
) -> String {
    let path_display = path.display();
    let old_text_trimmed = old_text.trim();
    let target_lines: Vec<&str> = old_text.lines().collect();
    let file_lines: Vec<&str> = file_content.lines().collect();

    // 1. Check for whitespace/newline normalized match
    let normalized_file_content: String = file_content
        .lines()
        .map(|l| l.split_whitespace().collect::<Vec<_>>().join(" "))
        .collect::<Vec<_>>()
        .join("\n");
    let normalized_old_text: String = old_text
        .lines()
        .map(|l| l.split_whitespace().collect::<Vec<_>>().join(" "))
        .collect::<Vec<_>>()
        .join("\n");

    if !normalized_old_text.is_empty() && normalized_file_content.contains(&normalized_old_text) {
        // Find approximate line number
        let mut match_line = 1;
        for (i, line) in file_lines.iter().enumerate() {
            let norm_line = line.split_whitespace().collect::<Vec<_>>().join(" ");
            let first_norm_target = target_lines.first().map(|l| l.split_whitespace().collect::<Vec<_>>().join(" ")).unwrap_or_default();
            if !first_norm_target.is_empty() && norm_line.contains(&first_norm_target) {
                match_line = i + 1;
                break;
            }
        }
        let end_line = (match_line + target_lines.len().saturating_sub(1)).max(match_line);

        return format!(
            "Error: patch hunk {hunk_index} old text was not found verbatim in {path_display}\n\n\
            [Self-Correction Guidance]:\n\
            • Cause: Whitespace or indentation mismatch detected around lines {match_line}–{end_line}.\n\
            • Diagnosis: The text exists in the file, but spaces, tabs, or line endings differ from your hunk.\n\
            • Recommended Action: Call 'read_file' with path=\"{path_display}\", start_line={match_line}, end_line={end_line} \
            to copy the exact verbatim indentation and whitespace before applying the patch."
        );
    }

    // 2. Sliding window similarity search across lines
    let window_size = target_lines.len().clamp(1, 40);
    let mut best_similarity = 0.0;
    let mut best_line = 1;

    if file_lines.len() >= window_size && !old_text_trimmed.is_empty() {
        for i in 0..=(file_lines.len().saturating_sub(window_size)) {
            let window_text = file_lines[i..i + window_size].join("\n");
            let sim = jaccard_similarity(old_text_trimmed, &window_text);
            if sim > best_similarity {
                best_similarity = sim;
                best_line = i + 1;
            }
        }
    }

    if best_similarity >= 0.5 {
        let pct = (best_similarity * 100.0).round() as u32;
        let end_line = best_line + window_size.saturating_sub(1);
        let suggested_end = end_line + 5;
        let sample_snippet = file_lines[best_line.saturating_sub(1)..best_line.saturating_sub(1) + window_size.min(4)].join("\n");

        format!(
            "Error: patch hunk {hunk_index} old text was not found in {path_display}\n\n\
            [Self-Correction Guidance]:\n\
            • Cause: Target code has drifted or modified. Closest match found at lines {best_line}–{end_line} ({pct}% similarity).\n\
            • Current File Snippet (lines {best_line}+):\n\
            ```\n\
            {sample_snippet}\n\
            ```\n\
            • Recommended Action: Call 'read_file' with path=\"{path_display}\", start_line={best_line}, end_line={suggested_end} \
            to review current contents, then re-create the patch hunk with the actual code."
        )
    } else {
        let total_lines = file_lines.len();
        format!(
            "Error: patch hunk {hunk_index} old text was not found in {path_display} ({total_lines} lines total)\n\n\
            [Self-Correction Guidance]:\n\
            • Cause: The specified old text does not match any region in '{path_display}'.\n\
            • Recommended Action: Call 'read_file' on '{path_display}' to locate the target section, or use 'search_code' \
            to check if the target function or variable was moved to another file."
        )
    }
}

/// Formats ambiguous patch matches with exact line numbers.
pub fn format_ambiguous_patch_guidance(
    file_content: &str,
    old_text: &str,
    hunk_index: usize,
    path: &Path,
    occurrences: usize,
) -> String {
    let path_display = path.display();
    let mut matching_lines = Vec::new();
    let old_first_line = old_text.lines().next().unwrap_or("").trim();

    for (idx, line) in file_content.lines().enumerate() {
        if line.contains(old_first_line) && !old_first_line.is_empty() {
            matching_lines.push(idx + 1);
        }
    }

    let lines_str = if matching_lines.is_empty() {
        format!("{occurrences} different locations")
    } else {
        matching_lines
            .iter()
            .take(6)
            .map(|l| format!("line {l}"))
            .collect::<Vec<_>>()
            .join(", ")
    };

    format!(
        "Error: patch hunk {hunk_index} old text matches {occurrences} locations in {path_display} ({lines_str})\n\n\
        [Self-Correction Guidance]:\n\
        • Cause: Non-unique replacement target. The model provided too few surrounding context lines.\n\
        • Recommended Actions:\n\
          1. To modify a single specific location: add 2–3 unique lines of surrounding code above and below 'old_text'.\n\
          2. To replace every occurrence across the file: set 'replaceAll': true on the hunk."
    )
}

/// Analyzes shell execution output and extracts language-specific compiler/runtime error pointers.
pub fn diagnose_command_failure(command: &str, output: &str, exit_code: Option<i32>) -> Option<String> {
    let code_str = exit_code.map(|c| format!("exit code {c}")).unwrap_or_else(|| "non-zero exit".into());

    // 1. Rust compiler error detection (rustc / cargo)
    if command.contains("cargo") || output.contains("error[E") {
        for line in output.lines() {
            if line.contains("error[E") {
                let err_msg = line.trim();
                // Find file pointer in subsequent lines
                let location = output.lines().find(|l| l.trim().starts_with("--> "));
                let loc_str = location
                    .map(|l| format!(" at {}", l.trim().trim_start_matches("--> ")))
                    .unwrap_or_default();

                return Some(format!(
                    "[Self-Correction Guidance — Rust Compilation Error ({code_str})]:\n\
                    • Error: {err_msg}{loc_str}\n\
                    • Recommended Action: Inspect the file and line indicated above using 'read_file', fix the syntax/type error using 'apply_patch', and re-run the build command."
                ));
            }
        }
    }

    // 2. TypeScript / Node compiler error detection
    if command.contains("tsc") || output.contains("error TS") {
        for line in output.lines() {
            if line.contains("error TS") {
                let err_msg = line.trim();
                return Some(format!(
                    "[Self-Correction Guidance — TypeScript Error ({code_str})]:\n\
                    • Error: {err_msg}\n\
                    • Recommended Action: Open the offending file with 'read_file', correct the typing issue, and verify with 'npm run typecheck' or 'tsc'."
                ));
            }
        }
    }

    // 3. Python Traceback detection
    if output.contains("Traceback (most recent call last):") {
        let lines: Vec<&str> = output.lines().collect();
        let last_exc = lines.iter().rev().find(|l| {
            l.contains("Error:") || l.contains("Exception:") || l.starts_with("SyntaxError")
        });
        let last_file_line = lines.iter().rev().find(|l| l.trim().starts_with("File \""));

        if let Some(exc) = last_exc {
            let loc = last_file_line.map(|l| format!(" ({})", l.trim())).unwrap_or_default();
            return Some(format!(
                "[Self-Correction Guidance — Python Runtime Exception ({code_str})]:\n\
                • Exception: {}{loc}\n\
                • Recommended Action: Read the traceback location using 'read_file', patch the root cause, and re-run your test command.",
                exc.trim()
            ));
        }
    }

    None
}

/// Computes token-level Jaccard similarity between two strings.
fn jaccard_similarity(a: &str, b: &str) -> f64 {
    use std::collections::BTreeSet;
    let tokens_a: BTreeSet<&str> = a.split_whitespace().collect();
    let tokens_b: BTreeSet<&str> = b.split_whitespace().collect();

    if tokens_a.is_empty() && tokens_b.is_empty() {
        return 1.0;
    }
    if tokens_a.is_empty() || tokens_b.is_empty() {
        return 0.0;
    }

    let intersection_count = tokens_a.intersection(&tokens_b).count();
    let union_count = tokens_a.union(&tokens_b).count();

    intersection_count as f64 / union_count as f64
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn detects_whitespace_and_indentation_mismatch() {
        let file_content = "fn main() {\n    let count = 42;\n    println!(\"{}\", count);\n}\n";
        // Target has different indentation (2 spaces instead of 4)
        let old_text = "fn main() {\n  let count = 42;\n  println!(\"{}\", count);\n}\n";
        let path = PathBuf::from("src/main.rs");

        let guidance = format_patch_not_found_guidance(file_content, old_text, 1, &path);
        assert!(guidance.contains("Whitespace or indentation mismatch detected"));
        assert!(guidance.contains("Recommended Action"));
        assert!(guidance.contains("read_file"));
    }

    #[test]
    fn fuzzy_similarity_pinpoints_drift() {
        let file_content = "line 1\nline 2\nlet total = calculate_sum(items, tax);\nlet discount = 0.1;\nline 5\n";
        // Target text has slight drift
        let old_text = "let total = calculate_sum(items);\nlet discount = 0.1;";
        let path = PathBuf::from("src/lib.rs");

        let guidance = format_patch_not_found_guidance(file_content, old_text, 1, &path);
        assert!(guidance.contains("Closest match found at lines"));
        assert!(guidance.contains("similarity"));
    }

    #[test]
    fn formats_ambiguous_matches_with_lines() {
        let file_content = "let x = 1;\nlet y = 2;\nlet x = 1;\n";
        let old_text = "let x = 1;\n";
        let path = PathBuf::from("src/test.rs");

        let guidance = format_ambiguous_patch_guidance(file_content, old_text, 1, &path, 2);
        assert!(guidance.contains("matches 2 locations"));
        assert!(guidance.contains("line 1, line 3"));
        assert!(guidance.contains("replaceAll"));
    }

    #[test]
    fn extracts_rust_compiler_error_diagnostics() {
        let output = "\
error[E0425]: cannot find value `foo` in this scope
  --> src/main.rs:12:5
   |
12 |     foo();
   |     ^^^ not found in this scope
";
        let diag = diagnose_command_failure("cargo check", output, Some(101));
        assert!(diag.is_some());
        let msg = diag.unwrap();
        assert!(msg.contains("Rust Compilation Error"));
        assert!(msg.contains("cannot find value `foo` in this scope"));
        assert!(msg.contains("src/main.rs:12:5"));
    }

    #[test]
    fn extracts_python_traceback_diagnostics() {
        let output = "\
Traceback (most recent call last):
  File \"app/server.py\", line 45, in run
    raise ValueError(\"invalid port\")
ValueError: invalid port
";
        let diag = diagnose_command_failure("python3 app/server.py", output, Some(1));
        assert!(diag.is_some());
        let msg = diag.unwrap();
        assert!(msg.contains("Python Runtime Exception"));
        assert!(msg.contains("ValueError: invalid port"));
        assert!(msg.contains("app/server.py"));
    }
}

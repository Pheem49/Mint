//! Line-aware Head-Tail Observation Truncation Harness
//!
//! Preserves the top execution context (command invocation, parameters, early progress)
//! and trailing execution context (stack traces, compiler errors, exit summaries)
//! while omitting intermediate verbose output with a structured diagnostic banner.
//! Supports file-backed scratchpad caching for long outputs (Cursor / Claude Code pattern).

use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};

/// Threshold in characters beyond which raw tool outputs are cached to disk
/// in `.mint/scratch/tool_outputs/` before being head-tail truncated.
pub const SCRATCHPAD_THRESHOLD_CHARS: usize = 3_000;

/// Configuration bounds for head-tail truncation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TruncationConfig {
    /// Maximum total characters permitted in the output (including banner).
    pub max_total_chars: usize,
    /// Maximum line count threshold before line-based truncation triggers.
    pub max_total_lines: usize,
    /// Number of initial lines to preserve from the beginning of output.
    pub head_lines: usize,
    /// Number of trailing lines to preserve from the end of output.
    pub tail_lines: usize,
}

impl Default for TruncationConfig {
    fn default() -> Self {
        Self {
            max_total_chars: 8_000,
            max_total_lines: 150,
            head_lines: 35,
            tail_lines: 75,
        }
    }
}

impl TruncationConfig {
    /// Compact configuration for text-rendered context previews and transcript exports.
    pub const COMPACT: Self = Self {
        max_total_chars: 2_500,
        max_total_lines: 60,
        head_lines: 15,
        tail_lines: 35,
    };
}

/// Saves raw tool output to `.mint/scratch/tool_outputs/<timestamp>_<action>_<hash>.log`
/// if `output` length exceeds `threshold_chars`.
/// Returns the relative path from workspace `root` (or absolute path if relative cannot be computed).
pub fn save_large_tool_output_scratchpad(
    root: &Path,
    action: &str,
    output: &str,
    threshold_chars: usize,
) -> Option<PathBuf> {
    if output.chars().count() <= threshold_chars {
        return None;
    }

    let scratch_dir = root.join(".mint").join("scratch").join("tool_outputs");
    if std::fs::create_dir_all(&scratch_dir).is_err() {
        return None;
    }

    let mut hasher = Sha256::new();
    hasher.update(output.as_bytes());
    let hash = format!("{:x}", hasher.finalize());
    let short_hash = if hash.len() >= 8 { &hash[..8] } else { &hash };

    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);

    let clean_action: String = action
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '_' || c == '-' {
                c
            } else {
                '_'
            }
        })
        .collect();

    let filename = format!("{timestamp}_{clean_action}_{short_hash}.log");
    let file_path = scratch_dir.join(&filename);

    if std::fs::write(&file_path, output).is_ok() {
        if let Ok(rel) = file_path.strip_prefix(root) {
            Some(rel.to_path_buf())
        } else {
            Some(file_path)
        }
    } else {
        None
    }
}

/// Truncates `text` using an industry-standard line-aware Head-Tail strategy.
///
/// If `scratch_path` is provided, a pointer to the on-disk full log is embedded in the omission banner.
pub fn head_tail_truncate_with_scratchpad(
    text: &str,
    config: TruncationConfig,
    scratch_path: Option<&Path>,
) -> String {
    if text.is_empty() {
        return String::new();
    }

    let char_count = text.chars().count();
    let lines: Vec<&str> = text.lines().collect();
    let total_lines = lines.len();

    // Fast path: text is already within both character and line bounds
    if char_count <= config.max_total_chars && total_lines <= config.max_total_lines {
        return text.to_owned();
    }

    // If total lines are fewer than or equal to head + tail, but char count is still exceeded,
    // we perform character-based head-tail truncation.
    if total_lines <= config.head_lines + config.tail_lines {
        return char_head_tail_truncate(text, config.max_total_chars, scratch_path);
    }

    let head_count = config.head_lines.min(total_lines);
    let tail_count = config
        .tail_lines
        .min(total_lines.saturating_sub(head_count));
    let omitted_lines = total_lines.saturating_sub(head_count + tail_count);

    let head_text = lines[..head_count].join("\n");
    let tail_text = lines[total_lines - tail_count..].join("\n");

    let omitted_chars =
        char_count.saturating_sub(head_text.chars().count() + tail_text.chars().count());

    let banner = if let Some(path) = scratch_path {
        format!(
            "\n\n[... Truncated {omitted_lines} lines ({omitted_chars} chars) to preserve context window — showing initial {head_count} and trailing {tail_count} lines. Full output saved to {} — use read_file or grep if detailed inspection is needed ...]\n\n",
            path.display()
        )
    } else {
        format!(
            "\n\n[... Truncated {omitted_lines} lines ({omitted_chars} chars) to preserve context window — showing initial {head_count} and trailing {tail_count} lines ...]\n\n"
        )
    };

    let combined_chars =
        head_text.chars().count() + banner.chars().count() + tail_text.chars().count();
    if combined_chars <= config.max_total_chars {
        let mut result = String::with_capacity(head_text.len() + banner.len() + tail_text.len());
        result.push_str(&head_text);
        result.push_str(&banner);
        result.push_str(&tail_text);
        result
    } else {
        // If head + banner + tail still exceeds char limit, enforce character budget on the combined text
        let remaining_for_tail = config
            .max_total_chars
            .saturating_sub(head_text.chars().count() + banner.chars().count());
        let trimmed_tail: String = if remaining_for_tail > 0 {
            // Keep the very end of tail
            let tail_chars_count = tail_text.chars().count();
            let skip_chars = tail_chars_count.saturating_sub(remaining_for_tail);
            tail_text.chars().skip(skip_chars).collect()
        } else {
            String::new()
        };

        let mut result = String::with_capacity(head_text.len() + banner.len() + trimmed_tail.len());
        result.push_str(&head_text);
        result.push_str(&banner);
        result.push_str(&trimmed_tail);
        result
    }
}

/// Truncates `text` using an industry-standard line-aware Head-Tail strategy.
///
/// If `text` fits within both `max_total_chars` and `max_total_lines`, it is returned unchanged.
/// Otherwise, the first `head_lines` and last `tail_lines` are preserved, separated by a
/// clear omission marker indicating the omitted line and character counts.
pub fn head_tail_truncate(text: &str, config: TruncationConfig) -> String {
    head_tail_truncate_with_scratchpad(text, config, None)
}

/// Deep module interface: caches large outputs (> 3,000 chars) to `.mint/scratch/tool_outputs/`
/// if `root` is available, then truncates with a pointer banner.
pub fn truncate_tool_output(
    root: Option<&Path>,
    action: &str,
    output: &str,
    config: TruncationConfig,
) -> String {
    let scratch_path = root.and_then(|r| {
        save_large_tool_output_scratchpad(r, action, output, SCRATCHPAD_THRESHOLD_CHARS)
    });
    head_tail_truncate_with_scratchpad(output, config, scratch_path.as_deref())
}

/// Fallback character-based head-tail truncation when line count is small but individual lines are huge.
fn char_head_tail_truncate(text: &str, max_chars: usize, scratch_path: Option<&Path>) -> String {
    let char_count = text.chars().count();
    if char_count <= max_chars {
        return text.to_owned();
    }

    let head_budget = (max_chars / 3).min(1_000);
    let head: String = text.chars().take(head_budget).collect();

    let banner_placeholder_len = 160;
    let tail_budget = max_chars.saturating_sub(head_budget + banner_placeholder_len);
    let skip_count = char_count.saturating_sub(tail_budget);
    let tail: String = text.chars().skip(skip_count).collect();

    let omitted_chars = char_count.saturating_sub(head.chars().count() + tail.chars().count());
    let banner = if let Some(path) = scratch_path {
        format!(
            "\n\n[... Truncated {omitted_chars} chars to preserve context window — showing head and tail. Full output saved to {} ...]\n\n",
            path.display()
        )
    } else {
        format!(
            "\n\n[... Truncated {omitted_chars} chars to preserve context window — showing head and tail ...]\n\n"
        )
    };

    let mut result = String::with_capacity(head.len() + banner.len() + tail.len());
    result.push_str(&head);
    result.push_str(&banner);
    result.push_str(&tail);
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn leaves_short_text_untouched() {
        let short = "Line 1\nLine 2\nLine 3";
        assert_eq!(
            head_tail_truncate(short, TruncationConfig::default()),
            short
        );
    }

    #[test]
    fn truncates_oversized_line_counts_preserving_head_and_tail() {
        let mut lines = Vec::new();
        for i in 1..=300 {
            lines.push(format!("Step log line {i}: details here"));
        }
        let input = lines.join("\n");

        let config = TruncationConfig {
            max_total_chars: 10_000,
            max_total_lines: 100,
            head_lines: 10,
            tail_lines: 20,
        };

        let result = head_tail_truncate(&input, config);
        assert!(result.starts_with("Step log line 1:"));
        assert!(result.contains("Step log line 10:"));
        assert!(result.contains("Truncated 270 lines"));
        assert!(result.contains("Step log line 281:"));
        assert!(result.ends_with("Step log line 300: details here"));
    }

    #[test]
    fn preserves_utf8_multibyte_safety() {
        let thai_line = "การทำงานขั้นตอนที่สำคัญมากสำหรับการทดสอบระบบ";
        let mut lines = Vec::new();
        for i in 1..=200 {
            lines.push(format!("{i}. {thai_line}"));
        }
        let input = lines.join("\n");

        let config = TruncationConfig {
            max_total_chars: 2_000,
            max_total_lines: 50,
            head_lines: 5,
            tail_lines: 5,
        };

        let result = head_tail_truncate(&input, config);
        assert!(result.contains("1. การทำงานขั้นตอน"));
        assert!(result.contains("200. การทำงานขั้นตอน"));
        assert!(result.contains("Truncated"));
    }

    #[test]
    fn handles_single_huge_line_gracefully() {
        let huge_line = "x".repeat(15_000);
        let config = TruncationConfig {
            max_total_chars: 1_000,
            max_total_lines: 100,
            head_lines: 10,
            tail_lines: 10,
        };

        let result = head_tail_truncate(&huge_line, config);
        assert!(result.len() < 2_000);
        assert!(result.contains("Truncated"));
    }

    #[test]
    fn saves_scratchpad_for_large_outputs() {
        let root = std::env::temp_dir().join(format!("mint-scratch-test-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&root).unwrap();
        let large_output = "Line log entry\n".repeat(300); // ~4,500 chars

        let scratch = save_large_tool_output_scratchpad(&root, "run_shell", &large_output, 3_000);
        assert!(scratch.is_some());
        let rel_path = scratch.unwrap();
        assert!(
            rel_path
                .to_string_lossy()
                .contains(".mint/scratch/tool_outputs/")
        );
        assert!(rel_path.to_string_lossy().ends_with(".log"));

        let full_path = root.join(&rel_path);
        assert!(full_path.exists());
        let saved_content = std::fs::read_to_string(full_path).unwrap();
        assert_eq!(saved_content, large_output);
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn skips_scratchpad_for_small_outputs() {
        let root = std::env::temp_dir().join(format!("mint-scratch-test-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&root).unwrap();
        let small_output = "short output";

        let scratch = save_large_tool_output_scratchpad(&root, "run_shell", small_output, 3_000);
        assert!(scratch.is_none());
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn truncate_tool_output_embeds_scratch_path_in_banner() {
        let root = std::env::temp_dir().join(format!("mint-scratch-test-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&root).unwrap();
        let mut lines = Vec::new();
        for i in 1..=300 {
            lines.push(format!("cargo check compiler warning {i}"));
        }
        let input = lines.join("\n");

        let truncated = truncate_tool_output(
            Some(&root),
            "run_shell",
            &input,
            TruncationConfig::default(),
        );

        assert!(truncated.contains("Truncated"));
        assert!(truncated.contains("Full output saved to .mint/scratch/tool_outputs/"));
        assert!(truncated.contains("use read_file or grep"));
        let _ = std::fs::remove_dir_all(&root);
    }
}

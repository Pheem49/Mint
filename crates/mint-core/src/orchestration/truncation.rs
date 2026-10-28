//! Line-aware Head-Tail Observation Truncation Harness
//!
//! Preserves the top execution context (command invocation, parameters, early progress)
//! and trailing execution context (stack traces, compiler errors, exit summaries)
//! while omitting intermediate verbose output with a structured diagnostic banner.

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

/// Truncates `text` using an industry-standard line-aware Head-Tail strategy.
///
/// If `text` fits within both `max_total_chars` and `max_total_lines`, it is returned unchanged.
/// Otherwise, the first `head_lines` and last `tail_lines` are preserved, separated by a
/// clear omission marker indicating the omitted line and character counts.
pub fn head_tail_truncate(text: &str, config: TruncationConfig) -> String {
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
        return char_head_tail_truncate(text, config.max_total_chars);
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

    let banner = format!(
        "\n\n[... Truncated {omitted_lines} lines ({omitted_chars} chars) to preserve context window — showing initial {head_count} and trailing {tail_count} lines ...]\n\n"
    );

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

/// Fallback character-based head-tail truncation when line count is small but individual lines are huge.
fn char_head_tail_truncate(text: &str, max_chars: usize) -> String {
    let char_count = text.chars().count();
    if char_count <= max_chars {
        return text.to_owned();
    }

    let head_budget = (max_chars / 3).min(1_000);
    let head: String = text.chars().take(head_budget).collect();

    let banner_placeholder_len = 100;
    let tail_budget = max_chars.saturating_sub(head_budget + banner_placeholder_len);
    let skip_count = char_count.saturating_sub(tail_budget);
    let tail: String = text.chars().skip(skip_count).collect();

    let omitted_chars = char_count.saturating_sub(head.chars().count() + tail.chars().count());
    let banner = format!(
        "\n\n[... Truncated {omitted_chars} chars to preserve context window — showing head and tail ...]\n\n"
    );

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
}

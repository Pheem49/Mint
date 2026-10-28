//! Shapes what flows back into the main agent's context: capped, labelled,
//! and compact — a chatty sub-agent must not re-bloat the context that
//! delegation exists to protect.
use super::{AgentStatus, SwarmOutcome};

/// Most characters of one agent's result that reach the main agent.
pub const MAX_DOSSIER_CHARS: usize = 4_000;

pub fn cap_dossier(text: &str) -> String {
    crate::orchestration::truncate_for_context(text.trim(), MAX_DOSSIER_CHARS)
}

/// One-line preview (first non-empty line, bounded) for UI summaries.
pub fn summary_preview(text: &str) -> String {
    let line = text
        .lines()
        .map(str::trim)
        .find(|l| !l.is_empty())
        .unwrap_or("");
    if line.chars().count() <= 160 {
        line.to_string()
    } else {
        let mut s: String = line.chars().take(160).collect();
        s.push_str("...");
        s
    }
}

/// `400ms`, `12s`, `1m 05s`.
pub fn format_elapsed_ms(ms: u64) -> String {
    if ms < 1_000 {
        return format!("{ms}ms");
    }
    let secs = ms / 1_000;
    if secs < 60 {
        format!("{secs}s")
    } else {
        format!("{}m {:02}s", secs / 60, secs % 60)
    }
}

pub fn format_outcome(outcome: &SwarmOutcome) -> String {
    let steps = if outcome.steps == 1 {
        "1 step".to_string()
    } else {
        format!("{} steps", outcome.steps)
    };
    format!(
        "[Agent '{}' · {} · {} · {}]\n{}",
        outcome.label,
        outcome.status.as_str(),
        format_elapsed_ms(outcome.elapsed_ms),
        steps,
        outcome.text
    )
}

/// The single tool result handed to the main agent for a whole batch.
pub fn format_batch_result(outcomes: &[SwarmOutcome]) -> String {
    let count = |s: AgentStatus| outcomes.iter().filter(|o| o.status == s).count();
    let mut text = format!(
        "Dispatched {} agents: {} completed / {} failed / {} cancelled.",
        outcomes.len(),
        count(AgentStatus::Completed),
        count(AgentStatus::Failed),
        count(AgentStatus::Cancelled),
    );
    for outcome in outcomes {
        text.push_str("\n\n");
        text.push_str(&format_outcome(outcome));
    }
    text
}

#[cfg(test)]
mod tests {
    use super::*;

    fn outcome(label: &str, status: AgentStatus, ms: u64, steps: u32, text: &str) -> SwarmOutcome {
        SwarmOutcome {
            label: label.into(),
            profile: "explorer".into(),
            status,
            elapsed_ms: ms,
            steps,
            text: text.into(),
        }
    }

    #[test]
    fn cap_dossier_trims_but_leaves_short_text_alone() {
        assert_eq!(cap_dossier("  findings  \n"), "findings");
    }

    #[test]
    fn cap_dossier_truncates_with_a_marker() {
        let capped = cap_dossier(&"x".repeat(20_000));
        assert!(capped.ends_with("... [truncated]"));
        assert!(capped.chars().count() <= MAX_DOSSIER_CHARS + "... [truncated]".len());
    }

    #[test]
    fn elapsed_formats_for_humans() {
        assert_eq!(format_elapsed_ms(400), "400ms");
        assert_eq!(format_elapsed_ms(12_000), "12s");
        assert_eq!(format_elapsed_ms(65_000), "1m 05s");
    }

    #[test]
    fn preview_is_the_first_non_empty_line_bounded() {
        assert_eq!(summary_preview("\n\n  first line \nsecond"), "first line");
        assert!(summary_preview(&"y".repeat(500)).chars().count() <= 160 + 3);
    }

    #[test]
    fn outcome_header_names_the_agent_status_time_and_steps() {
        let text = format_outcome(&outcome(
            "explorer-2",
            AgentStatus::Completed,
            12_000,
            5,
            "body",
        ));
        assert!(
            text.starts_with("[Agent 'explorer-2' · completed · 12s · 5 steps]"),
            "{text}"
        );
        assert!(text.ends_with("body"));
        let one = format_outcome(&outcome("a", AgentStatus::Failed, 1_000, 1, "x"));
        assert!(one.contains("1 step]"), "{one}");
    }

    #[test]
    fn batch_result_summarizes_counts_and_lists_every_agent_in_order() {
        let text = format_batch_result(&[
            outcome("explorer", AgentStatus::Completed, 1_000, 2, "found A"),
            outcome("debugger", AgentStatus::Failed, 2_000, 1, "boom"),
            outcome("explorer-2", AgentStatus::Cancelled, 0, 0, "not started"),
        ]);
        assert!(text.contains("3 agents"), "{text}");
        assert!(
            text.contains("1 completed")
                && text.contains("1 failed")
                && text.contains("1 cancelled"),
            "{text}"
        );
        let a = text.find("found A").unwrap();
        let b = text.find("boom").unwrap();
        let c = text.find("not started").unwrap();
        assert!(a < b && b < c, "agents must appear in task order");
    }
}

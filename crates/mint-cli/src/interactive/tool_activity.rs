//! Compact terminal tool cards, with unmodified input and output for review.
use mint_core::integrations::mcp_result::ToolStatus;
use ratatui::{
    style::{Color, Style},
    text::{Line, Span},
};
use serde_json::Value;
use std::time::{Duration, Instant};
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

#[derive(Debug)]
pub(super) struct ToolActivity {
    pub entry: usize,
    pub call_id: Option<String>,
    pub action: String,
    pub input: Value,
    pub subagent: Option<String>,
    pub started: Instant,
    pub elapsed: Option<Duration>,
    pub result: Option<String>,
    pub status: Option<ToolStatus>,
}

impl ToolActivity {
    pub fn running(&self) -> bool {
        self.status.is_none()
    }
    pub fn shell(&self) -> bool {
        matches!(
            self.action.as_str(),
            "run_shell" | "shell_output" | "kill_shell"
        )
    }
    pub fn matches(
        &self,
        call_id: Option<&str>,
        action: &str,
        input: &Value,
        subagent: Option<&str>,
    ) -> bool {
        if let Some(id) = call_id {
            self.call_id.as_deref() == Some(id) && self.subagent.as_deref() == subagent
        } else {
            self.action == action && self.input == *input && self.subagent.as_deref() == subagent
        }
    }
    pub fn finish(&mut self, result: &str, status: Option<ToolStatus>) {
        self.elapsed = Some(self.started.elapsed());
        self.result = Some(result.to_owned());
        self.status = Some(status.unwrap_or_else(|| {
            if result
                .trim_start()
                .to_ascii_lowercase()
                .starts_with("user denied")
            {
                return ToolStatus::Cancelled;
            }
            if result
                .trim_start()
                .to_ascii_lowercase()
                .starts_with("skipped")
            {
                return ToolStatus::Skipped;
            }
            if result.lines().any(|line| {
                line.strip_prefix("exit: ")
                    .is_some_and(|code| !matches!(code.trim(), "0" | "unknown"))
            }) || ["error:", "blocked:", "blocked by hook:"]
                .iter()
                .any(|prefix| result.trim_start().to_ascii_lowercase().starts_with(prefix))
            {
                ToolStatus::Failed
            } else {
                ToolStatus::Success
            }
        }));
    }
    pub fn heading(&self) -> String {
        let name = if self.shell() {
            "Shell".to_owned()
        } else {
            self.action.replace('_', " ")
        };
        let (icon, status) = match self.status {
            None => ("●", "running"),
            Some(ToolStatus::Success) => ("●", "completed"),
            Some(ToolStatus::Failed) => ("●", "failed"),
            Some(ToolStatus::TimedOut) => ("●", "timed out"),
            Some(ToolStatus::Cancelled)
                if self
                    .result
                    .as_deref()
                    .is_some_and(|text| text.starts_with("Interrupted")) =>
            {
                ("○", "interrupted")
            }
            Some(ToolStatus::Cancelled) => ("○", "cancelled"),
            Some(ToolStatus::Running) => ("◌", "running"),
            Some(ToolStatus::Unknown) => ("?", "unconfirmed"),
            Some(ToolStatus::Skipped) => ("○", "skipped"),
        };
        let elapsed = if self.running() {
            Some(self.started.elapsed())
        } else {
            self.elapsed
        };
        let duration = elapsed
            .map(|elapsed| format!("{:.1}s", elapsed.as_secs_f64()))
            .unwrap_or_else(|| "time unavailable".into());
        let mut title = format!("{icon} {name} {status} · {duration}");
        if let Some(result) = &self.result {
            let codes: Vec<_> = result
                .lines()
                .filter_map(|line| line.strip_prefix("exit: "))
                .collect();
            if !codes.is_empty() {
                title.push_str(&format!(" · exit {}", codes.join(", ")));
            }
        }
        if let Some(agent) = &self.subagent {
            title.push_str(&format!(" · {agent}"));
        }
        title
    }
    pub fn subject(&self) -> String {
        for key in [
            "description",
            "command",
            "cmd",
            "path",
            "query",
            "pattern",
            "url",
        ] {
            if let Some(value) = self.input.get(key).and_then(Value::as_str) {
                if let Some(line) = value.lines().find(|line| !line.trim().is_empty()) {
                    return line.trim().to_owned();
                }
            }
        }
        self.input.to_string()
    }
    pub fn lines(&self, width: u16) -> Vec<Line<'static>> {
        let output = self
            .result
            .as_ref()
            .map(|result| format!("Output · {} lines · ", result.lines().count()))
            .unwrap_or_default();
        vec![
            self.heading_line(width as usize),
            Line::styled(
                clip(&format!("  {}", self.subject()), width as usize),
                Style::default().fg(crate::terminal_theme::TEXT),
            ),
            Line::from(vec![
                Span::styled(
                    format!("  {output}"),
                    Style::default().fg(crate::terminal_theme::MUTED),
                ),
                Span::styled(
                    "Ctrl+O details",
                    Style::default().fg(crate::terminal_theme::BLUE),
                ),
            ]),
        ]
    }
    fn heading_line(&self, width: usize) -> Line<'static> {
        let color = match self.status {
            None => crate::terminal_theme::TEXT,
            Some(ToolStatus::Success) => crate::terminal_theme::ACCENT,
            Some(ToolStatus::Cancelled | ToolStatus::Skipped) => crate::terminal_theme::WARNING,
            _ => Color::Red,
        };
        let heading = clip(&self.heading(), width);
        let icon_end = heading.chars().next().map_or(0, char::len_utf8);
        Line::from(vec![
            Span::styled(heading[..icon_end].to_owned(), Style::default().fg(color)),
            Span::styled(
                heading[icon_end..].to_owned(),
                Style::default().fg(crate::terminal_theme::TEXT),
            ),
        ])
    }
    fn detail_input(&self) -> (bool, String) {
        let command = self
            .input
            .get("command")
            .or_else(|| self.input.get("cmd"))
            .and_then(Value::as_str);
        let input = command
            .map(str::to_owned)
            .unwrap_or_else(|| serde_json::to_string_pretty(&self.input).unwrap_or_default());
        (command.is_some(), input)
    }
    pub fn detail_lines(&self) -> Vec<Line<'static>> {
        use crate::terminal_theme::{ACCENT, BLUE, TEXT};
        let (command, input) = self.detail_input();
        let mut lines = vec![
            self.heading_line(usize::MAX),
            Line::styled(format!("Tool: {}", self.action), Style::default().fg(TEXT)),
            Line::default(),
            Line::styled(
                if command { "Command" } else { "Input" },
                Style::default().fg(BLUE),
            ),
        ];
        lines.extend(input.lines().map(|line| {
            Line::styled(
                line.to_owned(),
                Style::default().fg(if command { ACCENT } else { TEXT }),
            )
        }));
        lines.push(Line::default());
        lines.push(Line::styled("Output", Style::default().fg(BLUE)));
        lines.extend(
            self.result
                .as_deref()
                .unwrap_or("Waiting for result…")
                .lines()
                .map(|line| Line::styled(line.to_owned(), Style::default().fg(TEXT))),
        );
        lines
    }
    pub fn details(&self) -> String {
        let (command, input) = self.detail_input();
        format!(
            "{}\nTool: {}\n\n{}\n{}\n\nOutput\n{}",
            self.heading(),
            self.action,
            if command { "Command" } else { "Input" },
            input,
            self.result.as_deref().unwrap_or("Waiting for result…")
        )
    }
}

/// Pinned status: counts plus the newest running calls, independent of transcript scrolling.
pub(super) fn running_lines(tools: &[ToolActivity], width: usize, tick: usize) -> Vec<String> {
    let Some(summary) = running_summary(tools) else {
        return Vec::new();
    };
    let frames = ['⠋', '⠙', '⠹', '⠸', '⠼', '⠴', '⠦', '⠧', '⠇', '⠏'];
    let spinner = frames[tick % frames.len()];
    let mut lines = vec![clip(&format!("● {summary}"), width)];
    let mut running: Vec<_> = tools
        .iter()
        .rev()
        .filter(|tool| tool.running())
        .take(2)
        .collect();
    running.reverse();
    for tool in running {
        let prefix = format!("      {spinner} [{}] ", tool.action);
        let suffix = format!(" ({:.1}s)", tool.started.elapsed().as_secs_f64());
        let available = width.saturating_sub(
            UnicodeWidthStr::width(prefix.as_str()) + UnicodeWidthStr::width(suffix.as_str()),
        );
        let line = format!("{prefix}{}{suffix}", clip(&tool.subject(), available));
        lines.push(clip(&line, width));
    }
    lines
}

pub(super) fn running_summary(tools: &[ToolActivity]) -> Option<String> {
    let shells = tools
        .iter()
        .filter(|tool| tool.running() && tool.shell())
        .count();
    let other = tools
        .iter()
        .filter(|tool| tool.running() && !tool.shell())
        .count();
    let shell_label = format!(
        "{shells} shell command{}",
        if shells == 1 { "" } else { "s" }
    );
    let other_label = format!("{other} tool{}", if other == 1 { "" } else { "s" });
    match (shells, other) {
        (0, 0) => None,
        (_, 0) => Some(format!("Running {shell_label}")),
        (0, _) => Some(format!("Running {other_label}")),
        _ => Some(format!(
            "Running {} tasks · {shell_label} · {other_label}",
            shells + other
        )),
    }
}

fn clip(text: &str, width: usize) -> String {
    if UnicodeWidthStr::width(text) <= width {
        return text.to_owned();
    }
    let mut used = 0;
    let mut clipped = String::new();
    for ch in text.chars() {
        let next = ch.width().unwrap_or(0);
        if used + next > width.saturating_sub(1) {
            break;
        }
        used += next;
        clipped.push(ch);
    }
    if width > 0 {
        clipped.push('…');
    }
    clipped
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn pinned_running_status_animates_and_clips_commands_without_losing_timer() {
        let mut tool = ToolActivity {
            entry: 0,
            call_id: None,
            action: "run_shell".into(),
            input: serde_json::json!({"command":"x".repeat(300)}),
            subagent: None,
            started: Instant::now() - Duration::from_secs(3),
            elapsed: None,
            result: None,
            status: None,
        };
        let first = running_lines(std::slice::from_ref(&tool), 60, 0);
        let next = running_lines(std::slice::from_ref(&tool), 60, 1);
        assert!(first[1].contains('⠋'));
        assert!(next[1].contains('⠙'));
        assert!(first[1].ends_with("(3.0s)"));
        assert!(UnicodeWidthStr::width(first[1].as_str()) <= 60);
        tool.finish("exit: 0", None);
        assert!(running_lines(&[tool], 60, 2).is_empty());
    }

    #[test]
    fn blocked_tools_are_failed_instead_of_successful() {
        for result in ["Blocked: pinned server", "Blocked by hook: denied"] {
            let mut tool = ToolActivity {
                entry: 0,
                call_id: None,
                action: "read_file".into(),
                input: serde_json::json!({"path":"example"}),
                subagent: None,
                started: Instant::now(),
                elapsed: None,
                result: None,
                status: None,
            };
            tool.finish(result, None);
            assert!(matches!(tool.status, Some(ToolStatus::Failed)), "{result}");
            assert_eq!(tool.lines(80)[0].spans[0].style.fg, Some(Color::Red));
        }
    }

    #[test]
    fn denied_shell_command_is_cancelled_and_keeps_the_denial_in_details() {
        let mut tool = ToolActivity {
            entry: 0,
            call_id: None,
            action: "run_shell".into(),
            input: serde_json::json!({"command":"echo example"}),
            subagent: None,
            started: Instant::now(),
            elapsed: None,
            result: None,
            status: None,
        };
        tool.finish("User denied shell command: echo example", None);
        assert!(matches!(tool.status, Some(ToolStatus::Cancelled)));
        assert!(
            tool.details()
                .contains("User denied shell command: echo example")
        );
    }
}

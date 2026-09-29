//! CommandUi abstraction for slash commands and skill interaction.
//! Provides unified interaction for FullScreen (Ratatui TUI) and Classic (stdout/stdin) modes.

use super::chat_tui::{ChatTui, ChatViewState, TuiHandle};
use crate::{DIM, ERROR, RESET};
use anyhow::Result;
use std::sync::{Arc, Mutex};

#[derive(Debug, Clone)]
pub struct ChoiceItem {
    pub label: String,
    pub description: Option<String>,
    pub value: String,
}

impl ChoiceItem {
    pub fn new(label: impl Into<String>, value: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            description: None,
            value: value.into(),
        }
    }

    pub fn with_description(
        label: impl Into<String>,
        description: impl Into<String>,
        value: impl Into<String>,
    ) -> Self {
        Self {
            label: label.into(),
            description: Some(description.into()),
            value: value.into(),
        }
    }
}

pub trait CommandUi {
    fn uses_inline_resume_picker(&self) -> bool {
        false
    }
    fn push_notice_str(&mut self, text: &str);
    fn push_command_output_str(&mut self, text: &str);
    fn set_status(&mut self, lines: Vec<String>);
    fn clear_status(&mut self);
    fn prompt_choice(
        &mut self,
        title: &str,
        body: &str,
        options: &[ChoiceItem],
    ) -> Result<Option<usize>>;
    fn prompt_resume_picker(
        &mut self,
        sessions: Vec<mint_core::ChatSession>,
        _current_dir: &std::path::Path,
        _current_branch: Option<String>,
        _active_chat_id: &str,
    ) -> Result<Option<String>> {
        let options: Vec<ChoiceItem> = sessions
            .iter()
            .map(|session| ChoiceItem::new(session.title.clone(), session.id.clone()))
            .collect();
        match self.prompt_choice("Resume Session", "Type to search", &options)? {
            Some(index) => Ok(options.get(index).map(|item| item.value.clone())),
            None => Ok(None),
        }
    }
    #[allow(dead_code)]
    fn prompt_multi_choice(
        &mut self,
        title: &str,
        body: &str,
        options: &[ChoiceItem],
    ) -> Result<Option<Vec<usize>>>;
    fn prompt_text(
        &mut self,
        title: &str,
        body: &str,
        initial: Option<&str>,
    ) -> Result<Option<String>>;
    fn prompt_confirm(&mut self, question: &str, default: bool) -> Result<bool>;
    fn report_error(&mut self, error: &anyhow::Error);
    fn clear_transcript(&mut self) {}
    fn reload_transcript(&mut self, _chat_id: &str, _workspace: &std::path::Path) {}
}

pub trait CommandUiExt {
    fn push_notice(&mut self, text: impl AsRef<str>);
    fn push_command_output(&mut self, text: impl AsRef<str>);
}

impl<T: ?Sized + CommandUi> CommandUiExt for T {
    fn push_notice(&mut self, text: impl AsRef<str>) {
        self.push_notice_str(text.as_ref());
    }
    fn push_command_output(&mut self, text: impl AsRef<str>) {
        self.push_command_output_str(text.as_ref());
    }
}

pub struct FullScreenCommandUi<'a> {
    terminal: &'a mut ChatTui,
    state: &'a Arc<Mutex<ChatViewState>>,
    handle: &'a TuiHandle,
}

impl<'a> FullScreenCommandUi<'a> {
    pub fn new(
        terminal: &'a mut ChatTui,
        state: &'a Arc<Mutex<ChatViewState>>,
        handle: &'a TuiHandle,
    ) -> Self {
        Self {
            terminal,
            state,
            handle,
        }
    }
}

impl CommandUi for FullScreenCommandUi<'_> {
    fn push_notice_str(&mut self, text: &str) {
        self.handle.push_notice(text);
    }

    fn push_command_output_str(&mut self, text: &str) {
        self.handle.push_command(text);
    }

    fn set_status(&mut self, lines: Vec<String>) {
        self.handle.set_status(lines);
    }

    fn clear_status(&mut self) {
        self.handle.clear_status();
    }

    fn prompt_choice(
        &mut self,
        title: &str,
        body: &str,
        options: &[ChoiceItem],
    ) -> Result<Option<usize>> {
        let labels = options
            .iter()
            .map(|opt| match &opt.description {
                Some(desc) => format!("{} — {desc}", opt.label),
                None => opt.label.clone(),
            })
            .collect::<Vec<_>>();
        self.terminal.prompt_choice(self.handle, title, body, labels)
    }

    fn prompt_resume_picker(
        &mut self,
        sessions: Vec<mint_core::ChatSession>,
        current_dir: &std::path::Path,
        current_branch: Option<String>,
        active_chat_id: &str,
    ) -> Result<Option<String>> {
        let selected = self.terminal.prompt_resume_picker(
            self.handle,
            sessions.clone(),
            current_dir,
            current_branch,
            active_chat_id,
        )?;
        Ok(selected.and_then(|index| sessions.get(index).map(|session| session.id.clone())))
    }

    fn prompt_multi_choice(
        &mut self,
        title: &str,
        body: &str,
        options: &[ChoiceItem],
    ) -> Result<Option<Vec<usize>>> {
        let labels = options
            .iter()
            .map(|opt| match &opt.description {
                Some(desc) => format!("{} — {desc}", opt.label),
                None => opt.label.clone(),
            })
            .collect::<Vec<_>>();
        self.terminal
            .prompt_multi_choice(self.handle, title, body, labels)
    }

    fn prompt_text(
        &mut self,
        title: &str,
        body: &str,
        initial: Option<&str>,
    ) -> Result<Option<String>> {
        self.terminal.prompt_text(self.handle, title, body, initial)
    }

    fn prompt_confirm(&mut self, question: &str, default: bool) -> Result<bool> {
        let options = vec![
            ChoiceItem::new("Yes", "yes"),
            ChoiceItem::new("No", "no"),
        ];
        let default_idx = if default { 0 } else { 1 };
        match self.prompt_choice(question, "", &options)? {
            Some(idx) => Ok(idx == 0),
            None => Ok(default_idx == 0),
        }
    }

    fn report_error(&mut self, error: &anyhow::Error) {
        self.handle.push_notice(format!("Error: {error}"));
    }

    fn clear_transcript(&mut self) {
        if let Ok(mut state) = self.state.lock() {
            state.clear_transcript();
        }
    }

    fn reload_transcript(&mut self, chat_id: &str, workspace: &std::path::Path) {
        if let Ok(mut state) = self.state.lock() {
            state.reload_transcript(chat_id, workspace);
        }
    }
}

#[derive(Default)]
pub struct ClassicCommandUi;

impl ClassicCommandUi {
    pub fn new() -> Self {
        Self
    }
}

impl CommandUi for ClassicCommandUi {
    fn uses_inline_resume_picker(&self) -> bool {
        true
    }

    fn push_notice_str(&mut self, text: &str) {
        println!("{DIM}{text}{RESET}");
    }

    fn push_command_output_str(&mut self, text: &str) {
        println!("{text}");
    }

    fn set_status(&mut self, lines: Vec<String>) {
        if !lines.is_empty() {
            println!("{DIM}[Status] {}{RESET}", lines.join(" · "));
        }
    }

    fn clear_status(&mut self) {}

    fn prompt_choice(
        &mut self,
        title: &str,
        _body: &str,
        options: &[ChoiceItem],
    ) -> Result<Option<usize>> {
        let labels = options
            .iter()
            .map(|opt| match &opt.description {
                Some(desc) => format!("{} — {desc}", opt.label),
                None => opt.label.clone(),
            })
            .collect::<Vec<_>>();
        let default_val = labels.first().map(|s| s.as_str()).unwrap_or("");
        match crate::interactive::prompt_interactive_select(title, &labels, default_val) {
            Ok(Some(choice)) => Ok(labels.iter().position(|l| l == &choice)),
            Ok(None) => Ok(None),
            Err(e) => Err(e),
        }
    }

    fn prompt_multi_choice(
        &mut self,
        title: &str,
        _body: &str,
        options: &[ChoiceItem],
    ) -> Result<Option<Vec<usize>>> {
        use std::io::Write;
        if options.is_empty() {
            return Ok(Some(Vec::new()));
        }
        println!("\n\x1b[38;2;78;201;216m{title}\x1b[0m");
        for (i, opt) in options.iter().enumerate() {
            if let Some(desc) = &opt.description {
                println!("  [{}] {} — \x1b[90m{desc}\x1b[0m", i + 1, opt.label);
            } else {
                println!("  [{}] {}", i + 1, opt.label);
            }
        }
        print!("Enter comma-separated numbers (or press Enter to cancel): ");
        std::io::stdout().flush()?;
        let mut line = String::new();
        std::io::stdin().read_line(&mut line)?;
        let trimmed = line.trim();
        if trimmed.is_empty() {
            return Ok(None);
        }
        let indices = trimmed
            .split(',')
            .filter_map(|s| s.trim().parse::<usize>().ok())
            .filter(|&num| num >= 1 && num <= options.len())
            .map(|num| num - 1)
            .collect();
        Ok(Some(indices))
    }

    fn prompt_text(
        &mut self,
        title: &str,
        body: &str,
        initial: Option<&str>,
    ) -> Result<Option<String>> {
        use std::io::Write;
        let prompt = if body.is_empty() { title } else { body };
        if let Some(init) = initial {
            print!("{prompt} [{init}]: ");
        } else {
            print!("{prompt}: ");
        }
        std::io::stdout().flush()?;
        let mut line = String::new();
        std::io::stdin().read_line(&mut line)?;
        let trimmed = line.trim();
        if trimmed.is_empty() {
            Ok(initial.map(str::to_string))
        } else {
            Ok(Some(trimmed.to_string()))
        }
    }

    fn prompt_confirm(&mut self, question: &str, default: bool) -> Result<bool> {
        let suffix = if default { "[Y/n]" } else { "[y/N]" };
        crate::interactive::confirm(&format!("{question} {suffix} "))
    }

    fn report_error(&mut self, error: &anyhow::Error) {
        eprintln!("{ERROR}Error:{RESET} {error}");
    }
}

#[allow(dead_code)]
#[derive(Default, Debug)]
pub struct MockCommandUi {
    pub notices: Vec<String>,
    pub command_outputs: Vec<String>,
    pub status: Vec<String>,
    pub choice_answers: Vec<Option<usize>>,
    pub text_answers: Vec<Option<String>>,
    pub confirm_answers: Vec<bool>,
}

#[allow(dead_code)]
impl MockCommandUi {
    pub fn new() -> Self {
        Self::default()
    }
}

impl CommandUi for MockCommandUi {
    fn push_notice_str(&mut self, text: &str) {
        self.notices.push(text.to_string());
    }

    fn push_command_output_str(&mut self, text: &str) {
        self.command_outputs.push(text.to_string());
    }

    fn set_status(&mut self, lines: Vec<String>) {
        self.status = lines;
    }

    fn clear_status(&mut self) {
        self.status.clear();
    }

    fn prompt_choice(
        &mut self,
        _title: &str,
        _body: &str,
        _options: &[ChoiceItem],
    ) -> Result<Option<usize>> {
        if !self.choice_answers.is_empty() {
            Ok(self.choice_answers.remove(0))
        } else {
            Ok(None)
        }
    }

    fn prompt_multi_choice(
        &mut self,
        _title: &str,
        _body: &str,
        _options: &[ChoiceItem],
    ) -> Result<Option<Vec<usize>>> {
        Ok(None)
    }

    fn prompt_text(
        &mut self,
        _title: &str,
        _body: &str,
        _initial: Option<&str>,
    ) -> Result<Option<String>> {
        if !self.text_answers.is_empty() {
            Ok(self.text_answers.remove(0))
        } else {
            Ok(None)
        }
    }

    fn prompt_confirm(&mut self, _question: &str, _default: bool) -> Result<bool> {
        if !self.confirm_answers.is_empty() {
            Ok(self.confirm_answers.remove(0))
        } else {
            Ok(true)
        }
    }

    fn report_error(&mut self, error: &anyhow::Error) {
        self.notices.push(format!("Error: {error}"));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mock_command_ui_flow() {
        let mut ui = MockCommandUi::new();
        ui.push_notice("Notice 1");
        ui.push_command_output("Output 1");
        assert_eq!(ui.notices, vec!["Notice 1"]);
        assert_eq!(ui.command_outputs, vec!["Output 1"]);

        ui.choice_answers.push(Some(2));
        let choice = ui.prompt_choice("Select", "Body", &[]).unwrap();
        assert_eq!(choice, Some(2));

        ui.text_answers.push(Some("hello".into()));
        let text = ui.prompt_text("Input", "Body", None).unwrap();
        assert_eq!(text, Some("hello".into()));

        ui.confirm_answers.push(false);
        let confirmed = ui.prompt_confirm("Confirm?", true).unwrap();
        assert!(!confirmed);
    }
}

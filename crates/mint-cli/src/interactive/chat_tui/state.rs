//! ChatViewState, TuiHandle, composer editing, tool progress, and SQLite transcript sync.
use super::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SwarmUiStatus {
    Running,
    Success,
    Failed,
}

#[derive(Debug, Clone)]
pub(crate) struct SwarmAgentUiState {
    pub label: String,
    pub profile: String,
    pub task: String,
    pub current_action: Option<String>,
    pub status: SwarmUiStatus,
    pub started_at: std::time::Instant,
    pub finished_elapsed_ms: Option<u64>,
    pub summary_preview: Option<String>,
}

pub(super) fn clip_text(text: &str, width: usize) -> String {
    super::format::truncate_to_width(text, width)
}

pub(super) fn format_subagent_tool_action(action: &str, input: &serde_json::Value) -> String {
    super::tool_activity::ToolActivity::subagent_action_summary(action, input)
}

#[derive(Debug, Default)]
pub(crate) struct ChatViewState {
    pub(crate) transcript: Vec<TranscriptEntry>,
    pub(crate) swarm_agents: Vec<SwarmAgentUiState>,
    pub(super) chat_id: String,
    pub(super) mods: Vec<serde_json::Value>,
    pub(super) mods_open: bool,
    pub(super) mod_focus: usize,
    pub(super) mod_input: Option<(String, String)>,
    pub(super) sync_cursor: i64,
    pub(super) last_interaction_id: i64,
    pub(super) composer: Vec<char>,
    pub(super) collapsed_pastes: Vec<CollapsedPaste>,
    pub(super) expanded_paste: Option<std::ops::Range<usize>>,
    pub(super) cursor: usize,
    pub(super) history: Vec<String>,
    pub(super) history_index: Option<usize>,
    pub(super) draft_before_history: Vec<char>,
    pub(super) slash_selected: usize,
    pub(super) status: Vec<String>,
    pub(super) model: String,
    pub(super) thinking_status: String,
    pub(super) provider: String,
    pub(super) workspace: String,
    pub(super) current_dir: std::path::PathBuf,
    pub(super) plan_mode: bool,
    pub(super) scroll_from_bottom: usize,
    pub(super) selection_mode: bool,
    pub(super) selection_anchor: usize,
    pub(super) selection_head: usize,
    pub(super) notice: Option<(String, std::time::Instant)>,
    pub(super) dialog: Option<DialogState>,
    pub(super) thought_modal: Option<ThoughtModalState>,
    pub(super) thoughts: Vec<ThinkingSegment>,
    pub(super) tools: Vec<super::tool_activity::ToolActivity>,
    pub(super) tool_viewer: Option<usize>,
    pub(super) watermark_frame: u8,
    pub(super) watermark_animating: bool,
    pub(crate) queued_count: usize,
    pub(crate) queued_preview: Option<String>,
    pub(crate) theme: crate::terminal_theme::TuiTheme,
    pub(super) assistant_pacer: Option<crate::agent::ProgressiveMarkdownPacer>,
    pub(super) active_assistant_entry: Option<usize>,
    pub(super) pending_post_assistant_notices: Vec<String>,
    pub(super) last_assistant_tick: Option<std::time::Instant>,
}

#[derive(Clone, Debug)]
pub(crate) struct TuiHandle {
    pub(crate) state: Arc<Mutex<ChatViewState>>,
    pub(super) interrupted: Arc<AtomicBool>,
    pub(super) queued: Arc<Mutex<Vec<String>>>,
}

impl TuiHandle {
    pub fn tool_progress(&self, progress: &mint_core::AgentProgress) {
        if let Ok(mut state) = self.state.lock() {
            state.record_tool_progress(progress);
        }
    }

    pub fn reset_thoughts(&self) {
        if let Ok(mut state) = self.state.lock() {
            state.finish_live_thought();
            state.thought_modal = None;
        }
    }

    pub fn append_thought_delta(&self, id: &str, delta: &str, elapsed: std::time::Duration) {
        if delta.is_empty() {
            return;
        }
        if let Ok(mut state) = self.state.lock() {
            if let Some(thought) = state.thoughts.last_mut()
                && thought.id.as_deref() == Some(id)
                && thought.updated_at.is_some()
            {
                thought.text.push_str(delta);
                thought.elapsed = elapsed.max(thought.elapsed());
                thought.updated_at = Some(std::time::Instant::now());
            } else {
                state.finish_live_thought();
                state.thoughts.push(ThinkingSegment {
                    id: Some(id.to_owned()),
                    text: delta.to_owned(),
                    elapsed,
                    elapsed_known: true,
                    updated_at: Some(std::time::Instant::now()),
                });
            }
            state.refresh_thought_modal();
        }
    }

    pub fn finish_thought(&self, id: Option<&str>, text: &str, elapsed: std::time::Duration) {
        if let Ok(mut state) = self.state.lock() {
            if let Some(thought) = state.thoughts.last_mut()
                && thought.updated_at.is_some()
                && (id.is_none() || thought.id.as_deref() == id)
            {
                thought.text = text.to_owned();
                thought.elapsed = elapsed;
                thought.updated_at = None;
            } else if !text.trim().is_empty() {
                state.finish_live_thought();
                state.thoughts.push(ThinkingSegment {
                    id: id.map(str::to_owned),
                    text: text.to_owned(),
                    elapsed,
                    elapsed_known: true,
                    updated_at: None,
                });
            } else {
                state.finish_live_thought();
                return;
            }
            state.push_notice(format!(
                "Thought · {} (Ctrl+T to view)",
                super::format_thought_elapsed(elapsed)
            ));
            state.refresh_thought_modal();
        }
    }

    pub fn set_status(&self, lines: Vec<String>) {
        if let Ok(mut state) = self.state.lock() {
            state.status = lines;
        }
    }
    pub fn clear_status(&self) {
        self.set_status(Vec::new());
    }
    pub fn push_notice(&self, text: impl Into<String>) {
        if let Ok(mut state) = self.state.lock() {
            state.push_notice(text);
        }
    }
    pub fn append_assistant_delta(&self, delta: &str) {
        if delta.is_empty() {
            return;
        }
        if let Ok(mut state) = self.state.lock() {
            state.append_assistant_delta(delta);
        }
    }
    pub fn begin_assistant_reveal(&self, summary: String, post_notices: Vec<String>) {
        if let Ok(mut state) = self.state.lock() {
            state.begin_assistant_reveal(summary, post_notices);
        }
    }
    pub fn push_command(&self, text: impl Into<String>) {
        if let Ok(mut state) = self.state.lock() {
            state
                .transcript
                .push(TranscriptEntry::new(TranscriptRole::Command, text));
            state.scroll_from_bottom = 0;
        }
    }
    #[allow(dead_code)]
    pub fn push_system(&self, text: impl Into<String>) {
        if let Ok(mut state) = self.state.lock() {
            state
                .transcript
                .push(TranscriptEntry::new(TranscriptRole::System, text));
            state.scroll_from_bottom = 0;
        }
    }
    pub fn set_notice(&self, text: impl Into<String>) {
        if let Ok(mut state) = self.state.lock() {
            state.set_notice(text);
        }
    }
    pub fn take_interrupted(&self) -> bool {
        self.interrupted.swap(false, Ordering::Relaxed)
    }
    pub fn take_queued(&self) -> Vec<String> {
        self.queued
            .lock()
            .map(|mut queued| std::mem::take(&mut *queued))
            .unwrap_or_default()
    }
    pub fn draft(&self) -> Option<String> {
        self.state
            .lock()
            .ok()
            .and_then(|state| (!state.composer.is_empty()).then(|| state.composer_text()))
    }
    pub fn choose(
        &self,
        title: impl Into<String>,
        body: impl Into<String>,
        options: Vec<String>,
    ) -> Option<usize> {
        let (reply, response) = mpsc::channel();
        if let Ok(mut state) = self.state.lock() {
            state.dialog = Some(DialogState::new_choice(title, body, options, None, reply));
        } else {
            return None;
        }
        match response.recv() {
            Ok(DialogAnswer::Choice(choice)) => Some(choice),
            _ => None,
        }
    }
    pub fn choose_many(
        &self,
        title: impl Into<String>,
        body: impl Into<String>,
        options: Vec<String>,
    ) -> Vec<usize> {
        let (reply, response) = mpsc::channel();
        let checked = vec![false; options.len()];
        if let Ok(mut state) = self.state.lock() {
            state.dialog = Some(DialogState::new_choice(
                title,
                body,
                options,
                Some(checked),
                reply,
            ));
        } else {
            return Vec::new();
        }
        match response.recv() {
            Ok(DialogAnswer::Choices(choices)) => choices,
            _ => Vec::new(),
        }
    }
    pub fn prompt_text(&self, title: impl Into<String>, body: impl Into<String>) -> Option<String> {
        let (reply, response) = mpsc::channel();
        if let Ok(mut state) = self.state.lock() {
            state.dialog = Some(DialogState::new_text(title, body, reply));
        } else {
            return None;
        }
        match response.recv() {
            Ok(DialogAnswer::Text(text)) if !text.trim().is_empty() => Some(text),
            _ => None,
        }
    }
}

impl ChatViewState {
    pub(super) fn record_tool_progress(
        &mut self,
        progress: &mint_core::AgentProgress,
    ) -> Option<usize> {
        use mint_core::AgentProgress;
        match progress {
            AgentProgress::SubagentStarted {
                subagent,
                profile,
                task,
            } => {
                if let Some(existing) = self.swarm_agents.iter_mut().find(|a| a.label == *subagent)
                {
                    existing.profile = profile.clone();
                    existing.task = task.clone();
                    existing.status = SwarmUiStatus::Running;
                    existing.started_at = std::time::Instant::now();
                } else {
                    self.swarm_agents.push(SwarmAgentUiState {
                        label: subagent.clone(),
                        profile: profile.clone(),
                        task: task.clone(),
                        current_action: None,
                        status: SwarmUiStatus::Running,
                        started_at: std::time::Instant::now(),
                        finished_elapsed_ms: None,
                        summary_preview: None,
                    });
                }
                return None;
            }
            AgentProgress::SubagentFinished {
                subagent,
                status,
                elapsed_ms,
                summary_preview,
                ..
            } => {
                let ui_status = if status == "failed" {
                    SwarmUiStatus::Failed
                } else {
                    SwarmUiStatus::Success
                };
                if let Some(existing) = self.swarm_agents.iter_mut().find(|a| a.label == *subagent)
                {
                    existing.status = ui_status;
                    existing.finished_elapsed_ms = Some(*elapsed_ms);
                    if !summary_preview.is_empty() {
                        existing.summary_preview = Some(summary_preview.clone());
                    }
                } else {
                    self.swarm_agents.push(SwarmAgentUiState {
                        label: subagent.clone(),
                        profile: String::new(),
                        task: String::new(),
                        current_action: None,
                        status: ui_status,
                        started_at: std::time::Instant::now(),
                        finished_elapsed_ms: Some(*elapsed_ms),
                        summary_preview: if summary_preview.is_empty() {
                            None
                        } else {
                            Some(summary_preview.clone())
                        },
                    });
                }
                return None;
            }
            AgentProgress::ToolStart {
                subagent: Some(agent_label),
                action,
                input,
                ..
            } => {
                if let Some(agent) = self
                    .swarm_agents
                    .iter_mut()
                    .find(|a| a.label == *agent_label)
                {
                    agent.current_action = Some(format_subagent_tool_action(action, input));
                }
                return None;
            }
            AgentProgress::ToolEnd {
                subagent: Some(_), ..
            } => {
                return None;
            }
            _ => {}
        }
        let (call_id, action, input, subagent, end) = match progress {
            AgentProgress::ToolStart {
                call_id,
                action,
                input,
                subagent,
            } => (call_id, action, input, subagent, None),
            AgentProgress::ToolEnd {
                call_id,
                action,
                input,
                subagent,
                result,
                status,
                elapsed_ms,
            } => (
                call_id,
                action,
                input,
                subagent,
                Some((result, status, elapsed_ms)),
            ),
            _ => return None,
        };
        let existing = self.tools.iter().position(|tool| {
            (end.is_some() || call_id.is_some())
                && tool.running()
                && tool.matches(call_id.as_deref(), action, input, subagent.as_deref())
        });
        let index = existing.unwrap_or_else(|| {
            let entry = self.transcript.len();
            self.transcript
                .push(TranscriptEntry::new(TranscriptRole::Notice, ""));
            self.tools.push(super::tool_activity::ToolActivity {
                entry,
                call_id: call_id.clone(),
                action: action.clone(),
                input: input.clone(),
                subagent: subagent.clone(),
                started: std::time::Instant::now(),
                elapsed: None,
                result: None,
                status: None,
            });
            self.tools.len() - 1
        });
        if let Some((result, status, elapsed_ms)) = end {
            self.tools[index].finish(result, status.clone());
            if let Some(elapsed_ms) = elapsed_ms {
                self.tools[index].elapsed = Some(std::time::Duration::from_millis(*elapsed_ms));
            }
        }
        self.refresh_tool_card(index);
        if self.tool_viewer == Some(index) {
            self.refresh_tool_viewer(false);
        }
        Some(index)
    }

    pub(super) fn refresh_tool_card(&mut self, index: usize) {
        let tool = &self.tools[index];
        let lines = tool.lines(80);
        let plain: Vec<String> = lines
            .iter()
            .map(|line| {
                line.spans
                    .iter()
                    .map(|span| span.content.as_ref())
                    .collect()
            })
            .collect();
        if let Some(entry) = self.transcript.get_mut(tool.entry) {
            *entry = TranscriptEntry {
                role: TranscriptRole::Notice,
                text: plain.join("\n"),
                rendered_lines: lines,
                plain_lines: plain,
                rendered_width: 80,
            };
        }
    }

    pub(super) fn finish_running_tools(&mut self) {
        for index in 0..self.tools.len() {
            if self.tools[index].running() {
                self.tools[index].finish(
                    "Interrupted before a result was received.",
                    Some(mint_core::integrations::mcp_result::ToolStatus::Cancelled),
                );
                self.refresh_tool_card(index);
            }
        }
        if self.tool_viewer.is_some() {
            self.refresh_tool_viewer(false);
        }
    }

    pub(super) fn refresh_tool_viewer(&mut self, reset_scroll: bool) {
        if self
            .dialog
            .as_ref()
            .is_some_and(|dialog| !dialog.is_tool_details())
        {
            self.tool_viewer = None;
            return;
        }
        let Some(index) = self.tool_viewer else {
            return;
        };
        let Some(tool) = self.tools.get(index) else {
            return;
        };
        let body = tool.details();
        if let Some(dialog) = self.dialog.as_mut() {
            dialog.title = format!("Tool details · {}/{}", index + 1, self.tools.len());
            dialog.body = body;
            if reset_scroll {
                dialog.body_scroll = 0;
            }
        } else {
            let (reply, _) = mpsc::channel();
            self.dialog = Some(DialogState::new_tool_details(
                format!("Tool details · {}/{}", index + 1, self.tools.len()),
                body,
                reply,
            ));
        }
    }

    pub(super) fn handle_tool_key(&mut self, key: event::KeyEvent) -> bool {
        if self
            .dialog
            .as_ref()
            .is_some_and(|dialog| !dialog.is_tool_details())
        {
            self.tool_viewer = None;
            return false;
        }
        use event::{KeyCode, KeyModifiers};
        if self.dialog.is_none() {
            self.tool_viewer = None;
        }
        let toggle =
            key.code == KeyCode::Char('o') && key.modifiers.contains(KeyModifiers::CONTROL);
        if let Some(index) = self.tool_viewer {
            if toggle || key.code == KeyCode::Esc {
                self.tool_viewer = None;
                self.dialog = None;
                return true;
            }
            if matches!(key.code, KeyCode::Tab | KeyCode::BackTab) {
                self.tool_viewer = Some(if key.code == KeyCode::Tab {
                    (index + 1) % self.tools.len()
                } else {
                    (index + self.tools.len() - 1) % self.tools.len()
                });
                self.refresh_tool_viewer(true);
                return true;
            }
        } else if toggle && self.dialog.is_none() && self.thought_modal.is_none() {
            if self.tools.is_empty() {
                self.set_notice("No tool activity recorded yet");
            } else {
                let target = self.preferred_tool_viewer_index();
                self.tool_viewer = Some(target);
                self.refresh_tool_viewer(true);
            }
            return true;
        }
        false
    }

    pub(super) fn preferred_tool_viewer_index(&self) -> usize {
        let has_recent_sources = self.transcript.iter().rev().take(6).any(|entry| {
            entry.text.starts_with("Sources:\n") || entry.text.contains("Ctrl+O view web_search")
        });

        if has_recent_sources {
            if let Some(pos) = self
                .tools
                .iter()
                .rposition(|tool| tool.action == "web_search")
            {
                return pos;
            }
        }
        self.tools.len() - 1
    }

    pub fn from_session(session: &InteractiveSession) -> Self {
        let mut state = Self {
            model: active_model(&session.config.ai_provider, &session.config).to_owned(),
            thinking_status: format_thinking_status(&session.config),
            provider: format_provider_display_name(&session.config.ai_provider, &session.config),
            workspace: format_workspace_with_branch(&session.current_dir),
            current_dir: session.current_dir.clone(),
            theme: crate::terminal_theme::TuiTheme::from_config(&session.config.tui_theme),
            plan_mode: session.plan_mode,
            history: session.history.clone(),
            ..Self::default()
        };
        state.reload_transcript(&session.chat_id, &session.current_dir);
        state
    }
    pub fn set_draft(&mut self, draft: String) {
        self.composer = normalize_prompt_newlines(&draft).chars().collect();
        self.collapsed_pastes.clear();
        self.expanded_paste = None;
        self.cursor = self.composer.len();
    }
    pub(super) fn composer_text(&self) -> String {
        self.composer_text_range(0..self.composer.len())
    }
    pub(super) fn composer_text_range(&self, range: std::ops::Range<usize>) -> String {
        let mut text = String::new();
        let mut start = range.start;
        for paste in self
            .collapsed_pastes
            .iter()
            .filter(|paste| paste.range.start >= range.start && paste.range.end <= range.end)
        {
            text.extend(self.composer[start..paste.range.start].iter());
            text.push_str(&paste.text);
            start = paste.range.end;
        }
        text.extend(self.composer[start..range.end].iter());
        text
    }
    pub(super) fn take_prompt(&mut self) -> String {
        let text = self.composer_text();
        self.composer.clear();
        self.collapsed_pastes.clear();
        self.expanded_paste = None;
        self.cursor = 0;
        text
    }
    pub(super) fn open_paste(&mut self) -> bool {
        let Some(index) = self
            .collapsed_pastes
            .iter()
            .enumerate()
            .min_by_key(|(_, paste)| {
                if self.cursor < paste.range.start {
                    paste.range.start - self.cursor
                } else {
                    self.cursor.saturating_sub(paste.range.end)
                }
            })
            .map(|(index, _)| index)
        else {
            return false;
        };
        self.expand_paste(index);
        true
    }
    pub(super) fn expand_paste(&mut self, index: usize) {
        let paste = self.collapsed_pastes.remove(index);
        let start = paste.range.start;
        self.replace_composer(paste.range, &paste.text);
        self.expanded_paste = Some(start..self.cursor);
        self.cursor = start;
    }
    pub(super) fn toggle_paste(&mut self) {
        if let Some(range) = self.expanded_paste.take() {
            let text = self.composer_text_range(range.clone());
            let start = range.start;
            let badge = format!("[Pasted Content {} chars]", text.chars().count());
            self.replace_composer(range, &badge);
            self.collapsed_pastes.push(CollapsedPaste {
                range: start..self.cursor,
                text,
            });
            self.collapsed_pastes.sort_by_key(|paste| paste.range.start);
        } else {
            self.open_paste();
        }
    }
    pub(super) fn replace_composer(&mut self, mut range: std::ops::Range<usize>, text: &str) {
        // Badges are atomic. Deleting any part removes the attached paste;
        // inserting inside one opens it before editing the actual text.
        if range.is_empty() {
            if let Some(index) = self
                .collapsed_pastes
                .iter()
                .position(|paste| range.start > paste.range.start && range.start < paste.range.end)
            {
                self.expand_paste(index);
                range = self.cursor..self.cursor;
            }
        } else {
            for paste in &self.collapsed_pastes {
                if range.start < paste.range.end && range.end > paste.range.start {
                    range.start = range.start.min(paste.range.start);
                    range.end = range.end.max(paste.range.end);
                }
            }
        }
        let start = range.start;
        let removed = range.end - start;
        let added = text.chars().count();
        self.collapsed_pastes.retain_mut(|paste| {
            if start < paste.range.end && range.end > paste.range.start {
                return false;
            }
            if paste.range.start >= range.end {
                paste.range.start = paste.range.start - removed + added;
                paste.range.end = paste.range.end - removed + added;
            }
            true
        });
        // Keep the open block's bounds attached to edits, including insertions
        // at either edge. Fully deleting it must also remove the F2 target.
        if let Some(open) = self.expanded_paste.as_mut() {
            if !range.is_empty() && start <= open.start && range.end >= open.end {
                self.expanded_paste = None;
            } else {
                if open.start > range.end || (!range.is_empty() && open.start == range.end) {
                    open.start = open.start - removed + added;
                } else if open.start >= start {
                    open.start = start;
                }
                if open.end >= range.end {
                    open.end = open.end - removed + added;
                } else if open.end >= start {
                    open.end = start + added;
                }
            }
        }
        self.composer.splice(range, text.chars());
        self.cursor = start + added;
        self.slash_selected = 0;
    }
    pub(super) fn edit_composer_key(&mut self, key: event::KeyEvent) -> bool {
        use event::{KeyCode, KeyModifiers};
        match key.code {
            KeyCode::F(2) => {
                self.toggle_paste();
            }
            KeyCode::Left => {
                self.cursor = self
                    .collapsed_pastes
                    .iter()
                    .find(|paste| self.cursor > paste.range.start && self.cursor <= paste.range.end)
                    .map_or(self.cursor.saturating_sub(1), |paste| paste.range.start);
            }
            KeyCode::Right => {
                self.cursor = self
                    .collapsed_pastes
                    .iter()
                    .find(|paste| self.cursor >= paste.range.start && self.cursor < paste.range.end)
                    .map_or((self.cursor + 1).min(self.composer.len()), |paste| {
                        paste.range.end
                    });
            }
            KeyCode::Backspace if self.cursor > 0 => {
                self.replace_composer(self.cursor - 1..self.cursor, "");
            }
            KeyCode::Delete if self.cursor < self.composer.len() => {
                self.replace_composer(self.cursor..self.cursor + 1, "");
            }
            KeyCode::Enter if key.modifiers.contains(KeyModifiers::ALT) => {
                self.replace_composer(self.cursor..self.cursor, "\n");
            }
            KeyCode::Char(character)
                if !key
                    .modifiers
                    .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT) =>
            {
                if self
                    .active_notice()
                    .is_some_and(|notice| !notice.starts_with("Terminal too small"))
                {
                    self.clear_notice();
                }
                self.replace_composer(self.cursor..self.cursor, &character.to_string());
            }
            _ => return false,
        }
        true
    }
    pub(super) fn insert_paste(&mut self, text: &str) {
        let text = normalize_prompt_newlines(text);
        if let Some(index) = self
            .collapsed_pastes
            .iter()
            .position(|paste| self.cursor > paste.range.start && self.cursor < paste.range.end)
        {
            self.expand_paste(index);
        }
        let cursor = self.cursor.min(self.composer.len());
        if text.lines().count() > 6 {
            let badge = format!("[Pasted Content {} chars]", text.chars().count());
            self.replace_composer(cursor..cursor, &badge);
            self.collapsed_pastes.push(CollapsedPaste {
                range: cursor..self.cursor,
                text: text.clone(),
            });
            self.collapsed_pastes.sort_by_key(|paste| paste.range.start);
        } else {
            self.replace_composer(cursor..cursor, &text);
        }
        self.slash_selected = 0;
    }
    pub(super) fn composer_width(&self, area: Rect) -> usize {
        let (main, _) = mod_layout(
            area,
            self.mods_open && self.dialog.is_none() && self.thought_modal.is_none(),
        );
        main.width.saturating_sub(4).max(1) as usize
    }
    pub(super) fn navigate_composer(&mut self, key: event::KeyEvent, row_width: usize) -> bool {
        use event::{KeyCode, KeyModifiers};
        if key
            .modifiers
            .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT)
        {
            return false;
        }
        // Honor the transcript's visible End shortcut before draft navigation.
        if key.code == KeyCode::End && self.scroll_from_bottom > 0 {
            self.scroll_from_bottom = 0;
            return true;
        }
        if self.composer.is_empty() {
            return false;
        }
        let (rows, row, col) = wrap_input_visual_into_rows(&self.composer, row_width, self.cursor);
        let target_row = match key.code {
            KeyCode::Up if rows.len() > 1 => row.saturating_sub(1),
            KeyCode::Down if rows.len() > 1 => (row + 1).min(rows.len() - 1),
            KeyCode::Home | KeyCode::End => row,
            _ => return false,
        };
        let start = input_row_start(&self.composer, &rows, target_row);
        let target: Vec<char> = rows[target_row].chars().collect();
        self.cursor = match key.code {
            KeyCode::Home => start,
            KeyCode::End => start + target.len(),
            _ => {
                let column = UnicodeWidthStr::width(
                    rows[row].chars().take(col).collect::<String>().as_str(),
                );
                let mut width = 0;
                let mut count = 0;
                for character in target {
                    let next = UnicodeWidthStr::width(character.to_string().as_str());
                    if width + next > column {
                        break;
                    }
                    width += next;
                    count += 1;
                }
                start + count
            }
        };
        true
    }
    pub fn reload_transcript(&mut self, chat_id: &str, workspace: &Path) {
        if self.chat_id != chat_id || self.current_dir != workspace {
            self.stop_mods();
        }
        if let Ok(memory) = mint_core::MemoryStore::open_default() {
            self.reload_transcript_with(&memory, chat_id, workspace);
        } else {
            self.chat_id = chat_id.to_owned();
            self.transcript.clear();
            self.tools.clear();
            if self
                .dialog
                .as_ref()
                .is_some_and(|dialog| dialog.is_tool_details())
            {
                self.dialog = None;
            }
            self.thoughts.clear();
            self.thought_modal = None;
            self.tool_viewer = None;
            self.assistant_pacer = None;
            self.active_assistant_entry = None;
            self.pending_post_assistant_notices.clear();
            self.last_interaction_id = 0;
            self.sync_cursor = 0;
        }
    }

    pub(super) fn push_saved_activity(
        &mut self,
        activity: Option<&serde_json::Value>,
        turn_status: &str,
    ) -> Vec<(String, String)> {
        let mut sources = Vec::new();
        let Some(events) = activity.and_then(serde_json::Value::as_array) else {
            return sources;
        };
        let first_tool = self.tools.len();
        for value in events {
            let Ok(event) = serde_json::from_value::<mint_core::AgentProgress>(value.clone())
            else {
                continue;
            };
            // In-flight external turns use the shared-sync placeholder until a terminal snapshot arrives.
            if !matches!(turn_status, "running" | "queued") {
                match &event {
                    mint_core::AgentProgress::ToolStart { .. } => {
                        self.record_tool_progress(&event);
                    }
                    mint_core::AgentProgress::ToolEnd { action, result, .. } => {
                        if let Some(index) = self.record_tool_progress(&event) {
                            self.tools[index].elapsed = value["data"]["elapsed_ms"]
                                .as_u64()
                                .or_else(|| value["data"]["elapsedMs"].as_u64())
                                .map(std::time::Duration::from_millis);
                            self.refresh_tool_card(index);
                        }
                        if action == "web_search"
                            && !result.starts_with("Web search error:")
                            && result != "No web search results found."
                        {
                            sources.extend(crate::agent::parse_web_search_sources(result));
                        }
                    }
                    mint_core::AgentProgress::SubagentStarted { .. }
                    | mint_core::AgentProgress::SubagentFinished { .. } => {
                        self.record_tool_progress(&event);
                    }
                    mint_core::AgentProgress::ExtendedThinking {
                        id,
                        thought,
                        elapsed_ms,
                    } if !thought.trim().is_empty() => {
                        self.thoughts.push(ThinkingSegment {
                            id: id.clone(),
                            text: thought.clone(),
                            elapsed: std::time::Duration::from_millis(elapsed_ms.unwrap_or(0)),
                            elapsed_known: elapsed_ms.is_some(),
                            updated_at: None,
                        });
                        let elapsed = self.thoughts.last().unwrap().elapsed_label();
                        self.push_notice(format!("Thought · {elapsed} (Ctrl+T to view)"));
                    }
                    mint_core::AgentProgress::Thought { thought } if !thought.trim().is_empty() => {
                        if mint_core::orchestration::is_internal_cot(thought) {
                            self.thoughts.push(ThinkingSegment {
                                id: None,
                                text: thought.clone(),
                                elapsed: std::time::Duration::ZERO,
                                elapsed_known: false,
                                updated_at: None,
                            });
                            self.push_notice("Thought · time unavailable (Ctrl+T to view)");
                        } else {
                            self.push_notice(thought.clone());
                        }
                    }
                    mint_core::AgentProgress::RunCompleted { summary } => {
                        self.push_notice(crate::agent::run_summary_lines(summary).join("\n"));
                    }
                    _ => {}
                }
            }
            // Suppress redundant ExecutionUpdate notices in TUI (handled by ToolActivity cards)
            if !matches!(event, mint_core::AgentProgress::ExecutionUpdate { .. }) {
                if let Some(notes) = crate::agent::mcp_progress_notes(&event) {
                    for note in notes {
                        self.push_notice(note);
                    }
                }
            }
        }
        for index in first_tool..self.tools.len() {
            if self.tools[index].running() {
                self.tools[index].finish(
                    "Interrupted before a result was recorded.",
                    Some(mint_core::integrations::mcp_result::ToolStatus::Cancelled),
                );
                self.tools[index].elapsed = None;
                self.refresh_tool_card(index);
            }
        }
        sources
    }

    pub(super) fn append_interaction_row(&mut self, row: &mint_core::InteractionMemory) {
        self.transcript
            .push(TranscriptEntry::new(TranscriptRole::User, &row.user_text));
        let sources = self.push_saved_activity(row.agent_activity.as_ref(), &row.status);
        match row.status.as_str() {
            "completed" => {
                self.transcript.push(TranscriptEntry::new(
                    TranscriptRole::Assistant,
                    &row.ai_text,
                ));
                if !sources.is_empty() {
                    let text = crate::agent::format_tui_web_sources(
                        &sources,
                        crate::agent::MAX_TUI_WEB_SOURCES,
                    );
                    if !text.is_empty() {
                        self.transcript
                            .push(TranscriptEntry::new(TranscriptRole::Notice, text));
                    }
                }
            }
            "queued" => self.transcript.push(TranscriptEntry::new(
                TranscriptRole::Notice,
                "Queued for this session…",
            )),
            "running" => self.transcript.push(TranscriptEntry::new(
                TranscriptRole::Notice,
                "Mint is responding…",
            )),
            "failed" => self.transcript.push(TranscriptEntry::new(
                TranscriptRole::Notice,
                "This turn failed. Send it again to retry.",
            )),
            _ => self.transcript.push(TranscriptEntry::new(
                TranscriptRole::Notice,
                "This turn was interrupted. Send it again to retry.",
            )),
        }
    }

    pub fn reload_transcript_with(
        &mut self,
        memory: &mint_core::MemoryStore,
        chat_id: &str,
        workspace: &Path,
    ) {
        if self.chat_id != chat_id || self.current_dir != workspace {
            self.stop_mods();
        }
        self.chat_id = chat_id.to_owned();
        self.transcript.clear();
        self.tools.clear();
        if self
            .dialog
            .as_ref()
            .is_some_and(|dialog| dialog.is_tool_details())
        {
            self.dialog = None;
        }
        self.thoughts.clear();
        self.thought_modal = None;
        self.tool_viewer = None;
        self.assistant_pacer = None;
        self.active_assistant_entry = None;
        self.pending_post_assistant_notices.clear();
        let scoped = mint_core::scoped_chat_id(chat_id, Some(&workspace.to_string_lossy()));
        self.sync_cursor = memory.latest_conversation_sequence(&scoped).unwrap_or(0);
        if let Ok(rows) = memory.interactions_for_chat(&scoped) {
            self.last_interaction_id = rows.last().map(|row| row.id).unwrap_or(0);
            for row in &rows {
                self.append_interaction_row(row);
            }
        } else {
            self.last_interaction_id = 0;
        }
    }

    pub fn advance_sync_cursor(&mut self) {
        if let Ok(memory) = mint_core::MemoryStore::open_default() {
            self.advance_sync_cursor_with(&memory);
        }
    }

    pub fn advance_sync_cursor_with(&mut self, memory: &mint_core::MemoryStore) {
        if self.chat_id.is_empty() {
            return;
        }
        let scoped =
            mint_core::scoped_chat_id(&self.chat_id, Some(&self.current_dir.to_string_lossy()));
        if let Ok(seq) = memory.latest_conversation_sequence(&scoped) {
            self.sync_cursor = seq;
        }
        if let Ok(rows) = memory.interactions_for_chat(&scoped) {
            if let Some(last) = rows.last() {
                self.last_interaction_id = last.id;
            }
        }
    }

    pub(super) fn refresh_shared_transcript(&mut self) -> bool {
        if self.chat_id.is_empty() {
            return false;
        }
        let Ok(memory) = mint_core::MemoryStore::open_default() else {
            return false;
        };
        self.refresh_shared_transcript_with(&memory)
    }

    pub fn refresh_shared_transcript_with(&mut self, memory: &mint_core::MemoryStore) -> bool {
        if self.chat_id.is_empty() {
            return false;
        }
        let scoped =
            mint_core::scoped_chat_id(&self.chat_id, Some(&self.current_dir.to_string_lossy()));
        let Ok(changes) = memory.conversation_changes(&scoped, self.sync_cursor, 200) else {
            return false;
        };
        if changes.changes.is_empty() {
            return false;
        }

        // If any interaction was deleted (e.g. /clear on Web or Desktop), do a full reload.
        if changes
            .changes
            .iter()
            .any(|change| change.interaction.is_none())
        {
            let chat_id = self.chat_id.clone();
            let workspace = self.current_dir.clone();
            self.reload_transcript_with(memory, &chat_id, &workspace);
            return true;
        }

        let mut changed = false;
        for change in &changes.changes {
            let Some(row) = &change.interaction else {
                continue;
            };
            if row.id > self.last_interaction_id {
                // Incoming new interaction from an external client (Web/Desktop)
                self.append_interaction_row(row);
                self.last_interaction_id = row.id;
                changed = true;
            } else if row.id == self.last_interaction_id {
                // Status update for an external interaction that was previously queued/running
                if row.status == "completed" {
                    let mut append = false;
                    if let Some(last) = self.transcript.last() {
                        if last.role == TranscriptRole::Notice
                            && (last.text.starts_with("Mint is responding")
                                || last.text.starts_with("Queued for this session"))
                        {
                            self.transcript.pop();
                            append = true;
                        } else if last.role != TranscriptRole::Assistant {
                            append = true;
                        }
                    }
                    if append {
                        let sources =
                            self.push_saved_activity(row.agent_activity.as_ref(), &row.status);
                        self.transcript.push(TranscriptEntry::new(
                            TranscriptRole::Assistant,
                            &row.ai_text,
                        ));
                        if !sources.is_empty() {
                            let text = crate::agent::format_tui_web_sources(
                                &sources,
                                crate::agent::MAX_TUI_WEB_SOURCES,
                            );
                            if !text.is_empty() {
                                self.transcript
                                    .push(TranscriptEntry::new(TranscriptRole::Notice, text));
                            }
                        }
                        changed = true;
                    }
                } else if row.status == "running" {
                    if let Some(last) = self.transcript.last_mut() {
                        if last.role == TranscriptRole::Notice
                            && last.text.starts_with("Queued for this session")
                        {
                            *last =
                                TranscriptEntry::new(TranscriptRole::Notice, "Mint is responding…");
                            changed = true;
                        }
                    }
                } else if matches!(row.status.as_str(), "failed" | "interrupted") {
                    let msg = if row.status == "failed" {
                        "This turn failed. Send it again to retry."
                    } else {
                        "This turn was interrupted. Send it again to retry."
                    };
                    if let Some(last) = self.transcript.last_mut() {
                        if last.role == TranscriptRole::Notice
                            && (last.text.starts_with("Mint is responding")
                                || last.text.starts_with("Queued for this session"))
                        {
                            *last = TranscriptEntry::new(TranscriptRole::Notice, msg);
                            changed = true;
                            self.push_saved_activity(row.agent_activity.as_ref(), &row.status);
                        }
                    }
                }
            }
        }

        self.sync_cursor = changes.cursor;
        changed
    }
    pub fn push_user(&mut self, text: String) {
        self.flush_assistant_reveal();
        self.transcript
            .push(TranscriptEntry::new(TranscriptRole::User, text));
        self.scroll_from_bottom = 0;
        self.swarm_agents.clear();
    }
    pub fn push_notice(&mut self, text: impl Into<String>) {
        let entry = TranscriptEntry::new(TranscriptRole::Notice, text);
        if let Some(idx) = self.active_assistant_entry
            && idx <= self.transcript.len()
        {
            self.transcript.insert(idx, entry);
            self.active_assistant_entry = Some(idx + 1);
            for tool in &mut self.tools {
                if tool.entry >= idx {
                    tool.entry += 1;
                }
            }
        } else {
            self.transcript.push(entry);
        }
        self.scroll_from_bottom = 0;
    }
    #[cfg(test)]
    pub fn push_assistant(&mut self, text: impl Into<String>) {
        self.flush_assistant_reveal();
        self.transcript
            .push(TranscriptEntry::new(TranscriptRole::Assistant, text));
        self.scroll_from_bottom = 0;
    }
    fn upsert_active_assistant_entry(&mut self, text: String) {
        if let Some(idx) = self.active_assistant_entry
            && idx < self.transcript.len()
            && self.transcript[idx].role == TranscriptRole::Assistant
        {
            self.transcript[idx] = TranscriptEntry::new(TranscriptRole::Assistant, text);
        } else {
            let idx = self.transcript.len();
            self.transcript
                .push(TranscriptEntry::new(TranscriptRole::Assistant, text));
            self.active_assistant_entry = Some(idx);
        }
    }
    pub fn append_assistant_delta(&mut self, delta: &str) {
        if delta.is_empty() {
            return;
        }
        let pacer = self
            .assistant_pacer
            .get_or_insert_with(crate::agent::ProgressiveMarkdownPacer::default);
        pacer.push_delta(delta);
        if self.active_assistant_entry.is_none() {
            self.tick_assistant_reveal();
        }
    }
    pub fn begin_assistant_reveal(&mut self, summary: String, post_notices: Vec<String>) {
        let pacer = self
            .assistant_pacer
            .get_or_insert_with(crate::agent::ProgressiveMarkdownPacer::default);
        pacer.finish_with_authoritative(&summary);
        self.pending_post_assistant_notices = post_notices;
        self.tick_assistant_reveal();
    }
    pub fn tick_assistant_reveal(&mut self) -> bool {
        let Some(mut pacer) = self.assistant_pacer.take() else {
            return false;
        };
        let revealed = pacer.step();
        let done = pacer.is_done();
        let mut changed = false;
        if let Some(text) = revealed {
            self.upsert_active_assistant_entry(text);
            self.last_assistant_tick = Some(std::time::Instant::now());
            changed = true;
        }
        if done {
            self.active_assistant_entry = None;
            self.last_assistant_tick = None;
            let notices = std::mem::take(&mut self.pending_post_assistant_notices);
            for notice in notices {
                self.transcript
                    .push(TranscriptEntry::new(TranscriptRole::Notice, notice));
                changed = true;
            }
        } else {
            self.assistant_pacer = Some(pacer);
        }
        changed
    }
    pub(super) fn tick_assistant_reveal_if_due(
        &mut self,
        min_interval: std::time::Duration,
    ) -> bool {
        if self.assistant_pacer.is_none() {
            return false;
        }
        if self
            .last_assistant_tick
            .is_some_and(|last| last.elapsed() < min_interval)
        {
            return false;
        }
        self.tick_assistant_reveal()
    }
    pub fn flush_assistant_reveal(&mut self) {
        if let Some(mut pacer) = self.assistant_pacer.take()
            && let Some(text) = pacer.flush_all()
        {
            self.upsert_active_assistant_entry(text);
        }
        self.active_assistant_entry = None;
        self.last_assistant_tick = None;
        let notices = std::mem::take(&mut self.pending_post_assistant_notices);
        for notice in notices {
            self.transcript
                .push(TranscriptEntry::new(TranscriptRole::Notice, notice));
        }
    }
    pub fn has_pending_assistant_reveal(&self) -> bool {
        self.assistant_pacer.is_some()
    }
    pub(super) fn has_unfinished_assistant_stream(&self) -> bool {
        self.assistant_pacer
            .as_ref()
            .is_some_and(|pacer| !pacer.is_completed())
    }
    pub fn clear_transcript(&mut self) {
        self.transcript.clear();
        self.tools.clear();
        self.swarm_agents.clear();
        self.assistant_pacer = None;
        self.active_assistant_entry = None;
        self.pending_post_assistant_notices.clear();
        if self
            .dialog
            .as_ref()
            .is_some_and(|dialog| dialog.is_tool_details())
        {
            self.dialog = None;
        }
        self.thoughts.clear();
        self.thought_modal = None;
        self.tool_viewer = None;
        self.scroll_from_bottom = 0;
        self.selection_mode = false;
    }
    pub fn sync_session(&mut self, session: &InteractiveSession) {
        if self.current_dir != session.current_dir {
            self.stop_mods();
        }
        self.model = active_model(&session.config.ai_provider, &session.config).to_owned();
        self.thinking_status = format_thinking_status(&session.config);
        self.provider = format_provider_display_name(&session.config.ai_provider, &session.config);
        self.workspace = format_workspace_with_branch(&session.current_dir);
        self.current_dir = session.current_dir.clone();
        self.plan_mode = session.plan_mode;
        self.history = session.history.clone();
    }
    pub(super) fn stop_mods(&mut self) {
        if !self.chat_id.is_empty() {
            let chat =
                mint_core::scoped_chat_id(&self.chat_id, Some(&self.current_dir.to_string_lossy()));
            mint_core::mods::close_chat(&self.current_dir, &chat);
        }
        self.mods.clear();
        self.mods_open = false;
    }
    pub(super) fn mod_input_value<'a>(&'a self, control: &'a crate::mods_ui::Control) -> &'a str {
        self.mod_input
            .as_ref()
            .filter(|(initial, _)| initial == &control.value)
            .map_or(&control.value, |(_, draft)| draft)
    }
    pub(super) fn mod_input_draft(&mut self, control: &crate::mods_ui::Control) -> &mut String {
        if self
            .mod_input
            .as_ref()
            .is_none_or(|(initial, _)| initial != &control.value)
        {
            self.mod_input = Some((control.value.clone(), control.value.clone()));
        }
        &mut self.mod_input.as_mut().unwrap().1
    }
    pub(super) fn history_previous(&mut self) {
        if self.history.is_empty() {
            return;
        }
        if self.history_index.is_none() {
            self.draft_before_history = self.composer_text().chars().collect();
        }
        let index = self
            .history_index
            .map(|index| index.saturating_sub(1))
            .unwrap_or(self.history.len() - 1);
        self.history_index = Some(index);
        self.set_draft(self.history[index].clone());
    }
    pub(super) fn history_next(&mut self) {
        let Some(index) = self.history_index else {
            return;
        };
        if index + 1 < self.history.len() {
            let next = index + 1;
            self.history_index = Some(next);
            self.set_draft(self.history[next].clone());
        } else {
            self.history_index = None;
            self.composer = std::mem::take(&mut self.draft_before_history);
        }
        self.cursor = self.composer.len();
    }
    pub(super) fn suggestions(&self) -> Vec<(String, String)> {
        match_composer_suggestions(&self.composer, self.cursor, &self.current_dir)
    }
    pub(super) fn apply_selected_suggestion(&mut self) -> bool {
        let suggestions = self.suggestions();
        let Some((token, _)) = suggestions.get(self.slash_selected % suggestions.len().max(1))
        else {
            return false;
        };
        let cursor = self.cursor.min(self.composer.len());
        let start = if token.starts_with('@') {
            self.composer[..cursor]
                .iter()
                .rposition(|c| c.is_whitespace())
                .map_or(0, |index| index + 1)
        } else {
            0
        };
        let word_end = self.composer[cursor..]
            .iter()
            .position(|c| c.is_whitespace())
            .map_or(self.composer.len(), |offset| cursor + offset);
        let paste_start = self
            .collapsed_pastes
            .iter()
            .find(|paste| paste.range.start >= cursor)
            .map(|paste| paste.range.start);
        let end = paste_start.map_or(word_end, |start| word_end.min(start));
        let needs_space = end == self.composer.len() || paste_start == Some(end);
        let replacement = if needs_space {
            format!("{token} ")
        } else {
            token.clone()
        };
        self.replace_composer(start..end, &replacement);
        if !needs_space {
            self.cursor += 1;
        }
        self.slash_selected = 0;
        true
    }
    pub fn set_notice(&mut self, text: impl Into<String>) {
        self.notice = Some((text.into(), std::time::Instant::now()));
    }
    pub fn clear_notice(&mut self) {
        self.notice = None;
    }
    pub fn active_notice(&self) -> Option<&str> {
        self.notice.as_ref().and_then(|(text, created)| {
            if text.starts_with("Terminal too small")
                || created.elapsed() < std::time::Duration::from_millis(2500)
            {
                Some(text.as_str())
            } else {
                None
            }
        })
    }
    pub(super) fn notice_visibility_changed_since_draw(&self, painted_visible: bool) -> bool {
        painted_visible != self.active_notice().is_some()
    }
    pub(super) fn transcript_text(&self, width: u16) -> Text<'static> {
        let mut lines = Vec::new();
        let selected_start = self.selection_anchor.min(self.selection_head);
        let selected_end = self.selection_anchor.max(self.selection_head);
        let mut line_index = 0usize;
        for (entry_index, entry) in self.transcript.iter().enumerate() {
            let role_header = match entry.role {
                TranscriptRole::User => None,
                TranscriptRole::Assistant => Some(("Mint", crate::terminal_theme::ACCENT)),
                TranscriptRole::Notice => None,
                TranscriptRole::Command => Some(("Command", crate::terminal_theme::ACCENT)),
                TranscriptRole::System => Some(("System", crate::terminal_theme::WARNING)),
            };
            if let Some((label, color)) = role_header {
                let selected =
                    self.selection_mode && (selected_start..=selected_end).contains(&line_index);
                let header = Line::from(Span::styled(
                    format!("{label} ›"),
                    Style::default()
                        .fg(color)
                        .add_modifier(Modifier::BOLD)
                        .add_modifier(if selected {
                            Modifier::REVERSED
                        } else {
                            Modifier::empty()
                        }),
                ));
                lines.push(header);
                line_index += 1;
            }
            let tool_lines = self
                .tools
                .iter()
                .find(|tool| tool.entry == entry_index)
                .map(|tool| tool.lines(width));
            let reflowed_entry_lines = entry.lines_for_width(width);
            let empty_user_line = [Line::default()];
            let content_lines = if let Some(lines) = tool_lines.as_ref() {
                lines.as_slice()
            } else if entry.role == TranscriptRole::User && reflowed_entry_lines.is_empty() {
                &empty_user_line[..]
            } else {
                reflowed_entry_lines.as_ref()
            };
            for (content_index, content_line) in content_lines.iter().enumerate() {
                let selected =
                    self.selection_mode && (selected_start..=selected_end).contains(&line_index);
                let mut sel_line = content_line.clone();
                if entry.role == TranscriptRole::User && content_index == 0 {
                    sel_line.spans.insert(
                        0,
                        Span::styled(
                            "You › ",
                            Style::default()
                                .fg(crate::terminal_theme::BLUE)
                                .add_modifier(Modifier::BOLD),
                        ),
                    );
                }
                if entry.role == TranscriptRole::Notice {
                    for span in &mut sel_line.spans {
                        if span.style.fg.is_none() {
                            span.style = span.style.fg(crate::terminal_theme::TEXT);
                        }
                    }
                }
                // Short AI text replies have no explicit ANSI color — default
                // them to plain white so they don't appear in the terminal's
                // dim default foreground.
                if entry.role == TranscriptRole::Assistant {
                    for span in &mut sel_line.spans {
                        if span.style.fg.is_none() {
                            span.style = span.style.fg(crate::terminal_theme::TEXT);
                        }
                    }
                }
                if selected {
                    for span in &mut sel_line.spans {
                        span.style = span.style.add_modifier(Modifier::REVERSED);
                    }
                }
                if entry.role == TranscriptRole::User {
                    lines.extend(highlighted_user_content(sel_line, width));
                } else {
                    lines.push(sel_line);
                }
                line_index += 1;
            }
            let selected =
                self.selection_mode && (selected_start..=selected_end).contains(&line_index);
            let mut style = Style::default();
            if selected {
                style = style.add_modifier(Modifier::REVERSED);
            }
            lines.push(Line::styled("", style));
            line_index += 1;
        }
        Text::from(lines)
    }
    pub(super) fn plain_transcript_lines(&self) -> Vec<String> {
        let mut lines = Vec::new();
        for entry in &self.transcript {
            if entry.role == TranscriptRole::User {
                let mut content = entry.plain_lines.iter();
                lines.push(format!(
                    "You › {}",
                    content.next().map_or("", String::as_str)
                ));
                lines.extend(content.cloned());
                lines.push(String::new());
                continue;
            }
            if let Some(header) = match entry.role {
                TranscriptRole::User => None,
                TranscriptRole::Assistant => Some("Mint ›"),
                TranscriptRole::Notice => None,
                TranscriptRole::Command => Some("Command ›"),
                TranscriptRole::System => Some("System ›"),
            } {
                lines.push(header.to_owned());
            }
            lines.extend(entry.plain_lines.iter().cloned());
            lines.push(String::new());
        }
        lines
    }
    pub(super) fn selected_text(&self) -> String {
        let lines = self.plain_transcript_lines();
        if lines.is_empty() {
            return String::new();
        }
        let start = self
            .selection_anchor
            .min(self.selection_head)
            .min(lines.len() - 1);
        let end = self
            .selection_anchor
            .max(self.selection_head)
            .min(lines.len() - 1);
        lines[start..=end].join("\n")
    }
    pub(super) fn mod_viewport(&self, area: Rect) -> mint_core::mods::Viewport {
        let (body, _) = mod_layout(
            area,
            self.mods_open && self.dialog.is_none() && self.thought_modal.is_none(),
        );
        mint_core::mods::Viewport {
            surface: Some("terminal".into()),
            columns: (body.width > 0).then_some(body.width),
            rows: (body.height > 0).then_some(body.height),
            side_panel_open: self.mods_open,
            ..Default::default()
        }
    }
}

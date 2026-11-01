//! Full-screen adapter for interactive chat.
use super::{
    InteractiveInput, InteractiveSession, active_model, format, format_provider_display_name,
    format_thought_elapsed, format_workspace_with_branch, tool_activity,
};
use anyhow::{Result, bail};
use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
use crossterm::{ExecutableCommand, event, terminal};
use ratatui::{
    Terminal,
    backend::CrosstermBackend,
    buffer::Buffer,
    layout::{Constraint, Direction, Layout, Position, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{
        Block, BorderType, Borders, Paragraph, Scrollbar, ScrollbarOrientation, ScrollbarState,
        Wrap,
    },
};
use std::io::{self, Write};
use std::path::Path;
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
    mpsc,
};
use unicode_width::UnicodeWidthStr;

use super::composer::normalize_prompt_newlines;

mod dialog;
mod render;
mod selection;
mod state;
mod thought;
mod transcript;
mod watermark;

#[cfg(test)]
mod tests;

#[cfg(test)]
pub(crate) use self::state::SwarmAgentUiState;
pub(crate) use self::{
    dialog::{DialogAnswer, DialogState},
    state::{ChatViewState, SwarmUiStatus, TuiHandle},
    thought::ThoughtModalState,
    transcript::{TranscriptEntry, TranscriptRole},
};
use self::{
    dialog::{resume_picker_lines, theme_picker_lines},
    render::{RenderAreas, mod_layout},
    selection::{
        CopyMethod, MouseSelection, ScreenPoint, TextClipboard, TranscriptSelection,
        apply_mouse_selection, input_index_at, selected_cells_match, selected_screen_text,
    },
    state::clip_text,
    thought::{ThinkingSegment, shimmer_thinking_line, thought_tail},
    transcript::{TranscriptLayout, highlighted_user_content},
    watermark::{WATERMARK_HEIGHT, WATERMARK_WIDTH, mint_m_watermark, watermark_animation_at},
};
#[cfg(test)]
use self::{selection::NativeClipboard, watermark::WATERMARK_FRAME_COUNT};
use super::composer::{
    CollapsedPaste, input_row_start, match_composer_suggestions, wrap_input_visual_into_rows,
};
use super::resume_picker::ResumePickerCore;

fn format_thinking_status(config: &mint_core::MintConfig) -> String {
    let model = active_model(&config.ai_provider, config);
    if !config.resolved_thinking_enabled_for_model(&config.ai_provider, model) {
        return "off".to_string();
    }
    match config.resolved_thinking_effort_for_model(model) {
        "extra_high" => "extra high".to_string(),
        effort => effort.to_string(),
    }
}

const MIN_WIDTH: u16 = 60;
const MIN_HEIGHT: u16 = 12;
const COPY_LIMIT: usize = 100 * 1024;
const BACK_TO_BOTTOM_LABEL: &str = " ↓ Back to bottom · End ";

pub(crate) struct ChatTui {
    terminal: Terminal<CrosstermBackend<io::Stdout>>,
    active: bool,
    mouse_selection: MouseSelection,
    transcript_selection: Option<TranscriptSelection>,
    transcript_area: Option<Rect>,
    selection_scroll_at: std::time::Instant,
    last_frame: Option<Buffer>,
    back_to_bottom_area: Option<Rect>,
    composer_area: Option<Rect>,
    watermark_area: Option<Rect>,
    clipboard: TextClipboard,
}

fn route_mouse_wheel(state: &mut ChatViewState, kind: event::MouseEventKind) {
    if let Some(dialog) = state.dialog.as_mut() {
        match kind {
            event::MouseEventKind::ScrollUp => {
                dialog.body_scroll = dialog
                    .body_scroll
                    .min(dialog.body_max_scroll.get())
                    .saturating_sub(3);
            }
            event::MouseEventKind::ScrollDown => {
                dialog.body_scroll = dialog
                    .body_scroll
                    .saturating_add(3)
                    .min(dialog.body_max_scroll.get());
            }
            _ => {}
        }
        return;
    }
    match kind {
        event::MouseEventKind::ScrollUp => {
            if let Some(modal) = state.thought_modal.as_mut() {
                modal.scroll = modal.scroll.saturating_sub(3);
            } else {
                state.scroll_from_bottom = state.scroll_from_bottom.saturating_add(3);
            }
        }
        event::MouseEventKind::ScrollDown => {
            if let Some(modal) = state.thought_modal.as_mut() {
                modal.scroll = modal.scroll.saturating_add(3);
            } else {
                state.scroll_from_bottom = state.scroll_from_bottom.saturating_sub(3);
            }
        }
        _ => {}
    }
}

fn activate_back_to_bottom(
    state: &mut ChatViewState,
    area: Option<Rect>,
    kind: event::MouseEventKind,
    point: ScreenPoint,
) -> bool {
    if kind == event::MouseEventKind::Down(event::MouseButton::Left)
        && area.is_some_and(|area| area.contains(Position::new(point.column, point.row)))
    {
        state.scroll_from_bottom = 0;
        true
    } else {
        false
    }
}

fn setup_terminal_output<W: Write>(stdout: &mut W) -> io::Result<()> {
    crossterm::execute!(
        stdout,
        terminal::EnterAlternateScreen,
        // Mouse capture lets Mint distinguish wheel scrolling from physical
        // Up/Down keys and powers Mint's own drag-to-copy selection layer.
        event::EnableMouseCapture,
        event::EnableBracketedPaste
    )
}

pub(crate) fn restore_terminal() {
    let mut stdout = io::stdout();
    let _ = stdout.execute(event::DisableBracketedPaste);
    let _ = stdout.execute(event::DisableMouseCapture);
    let _ = stdout.execute(terminal::LeaveAlternateScreen);
    let _ = terminal::disable_raw_mode();
}
impl ChatTui {
    pub fn enter(workspace: &Path) -> Result<Self> {
        use crossterm::tty::IsTty;
        if !io::stdin().is_tty() || !io::stdout().is_tty() {
            bail!("stdin/stdout is not a TTY");
        }
        let (w, h) = terminal::size()?;
        if w < MIN_WIDTH || h < MIN_HEIGHT {
            bail!("terminal is too small ({w}x{h}; need {MIN_WIDTH}x{MIN_HEIGHT})");
        }
        terminal::enable_raw_mode()?;
        let mut stdout = io::stdout();
        let workspace_name = workspace
            .file_name()
            .unwrap_or_else(|| workspace.as_os_str())
            .to_string_lossy();
        let workspace_name: String = workspace_name
            .chars()
            .map(|character| {
                if character.is_control() {
                    ' '
                } else {
                    character
                }
            })
            .collect();
        let title = if workspace_name.trim().is_empty() {
            "Mint Agent | workspace".to_owned()
        } else {
            format!("Mint Agent | {}", workspace_name.trim())
        };
        let setup = setup_terminal_output(&mut stdout);
        if let Err(error) = setup {
            restore_terminal();
            return Err(error.into());
        }
        match Terminal::new(CrosstermBackend::new(stdout)) {
            Ok(terminal) => {
                let _ = io::stdout().execute(terminal::SetTitle(title));
                Ok(Self {
                    terminal,
                    active: true,
                    mouse_selection: MouseSelection::default(),
                    transcript_selection: None,
                    transcript_area: None,
                    selection_scroll_at: std::time::Instant::now(),
                    last_frame: None,
                    back_to_bottom_area: None,
                    composer_area: None,
                    watermark_area: None,
                    clipboard: TextClipboard::default(),
                })
            }
            Err(error) => {
                restore_terminal();
                Err(error.into())
            }
        }
    }
    pub(super) fn draw_state(&mut self, state: &ChatViewState) -> Result<()> {
        let selection = self.mouse_selection;
        let previous = self.last_frame.as_ref();
        let mut transcript_selection = self.transcript_selection.as_mut();
        let mut invalidated = false;
        let mut areas = RenderAreas::default();
        let completed = self.terminal.draw(|frame| {
            areas = state.render(frame);
            if selection.range().is_some() && state.active_notice().is_none() {
                let area = frame.area();
                let hint_area = Rect::new(area.x, area.bottom().saturating_sub(1), area.width, 1);
                frame.render_widget(
                    Paragraph::new(Line::styled(
                        " Ctrl+C / right-click copy · Delete input · Esc clear",
                        Style::default().fg(crate::terminal_theme::MUTED),
                    )),
                    hint_area,
                );
            }
            invalidated = transcript_selection.is_none()
                && !selection.dragging
                && selection.range().is_some()
                && previous
                    .map(|buffer| !selected_cells_match(buffer, frame.buffer_mut(), selection))
                    .unwrap_or(false);
            if !invalidated {
                if let Some(selected) = transcript_selection.as_deref_mut() {
                    if let Some(area) = areas
                        .transcript
                        .filter(|area| area.width > 0 && area.height > 0)
                    {
                        selected.area = area;
                    }
                    selected.paint(frame.buffer_mut(), areas.back_to_bottom);
                } else {
                    apply_mouse_selection(frame.buffer_mut(), selection);
                }
            }
            crate::terminal_theme::apply_tui_theme(frame.buffer_mut(), state.theme);
        })?;
        self.last_frame = Some(completed.buffer.clone());
        self.transcript_area = areas.transcript;
        self.back_to_bottom_area = areas.back_to_bottom;
        self.composer_area = areas.composer;
        self.watermark_area = areas.watermark;
        if invalidated {
            self.clear_mouse_selection();
        }
        Ok(())
    }

    fn handle_selection_mouse(
        &mut self,
        mouse: event::MouseEvent,
        state: &mut ChatViewState,
    ) -> Result<bool> {
        use event::{MouseButton, MouseEventKind};

        let point = ScreenPoint::from_mouse(mouse);
        if activate_back_to_bottom(state, self.back_to_bottom_area, mouse.kind, point) {
            self.clear_mouse_selection();
            return Ok(true);
        }
        match mouse.kind {
            MouseEventKind::Down(MouseButton::Left) => {
                let inside_frame = self
                    .last_frame
                    .as_ref()
                    .map(|buffer| buffer.area.contains(Position::new(point.column, point.row)))
                    .unwrap_or(false);
                if inside_frame {
                    self.clear_mouse_selection();
                    self.mouse_selection.start(point);
                    if let Some(area) = self.transcript_area.filter(|area| {
                        area.width > 0
                            && area.height > 0
                            && area.contains(Position::new(point.column, point.row))
                    }) && state.dialog.is_none()
                        && state.thought_modal.is_none()
                        && !state.transcript.is_empty()
                    {
                        let text = state.transcript_text(area.width);
                        let layout = TranscriptLayout::new(text, area.width);
                        let max = layout.height.saturating_sub(area.height as usize);
                        let offset = max.saturating_sub(state.scroll_from_bottom.min(max));
                        self.transcript_selection = Some(TranscriptSelection::from_text(
                            layout.text,
                            area,
                            offset,
                            point,
                        ));
                    }
                    state.selection_mode = false;
                    state.clear_notice();
                } else {
                    self.clear_mouse_selection();
                }
                Ok(true)
            }
            MouseEventKind::Drag(MouseButton::Left) => {
                self.mouse_selection.update(point);
                if let Some(selected) = self.transcript_selection.as_mut() {
                    selected.update(point);
                }
                Ok(self.mouse_selection.dragging)
            }
            MouseEventKind::Up(MouseButton::Left) => {
                if let Some(selected) = self.transcript_selection.as_mut() {
                    selected.update(point);
                    selected.dragging = false;
                }
                if !self.mouse_selection.finish(point) {
                    self.transcript_selection = None;
                    return Ok(true);
                }
                if self
                    .last_frame
                    .as_ref()
                    .map(|buffer| {
                        self.transcript_selection
                            .as_ref()
                            .map_or_else(
                                || selected_screen_text(buffer, self.mouse_selection),
                                TranscriptSelection::text,
                            )
                            .trim()
                            .is_empty()
                    })
                    .unwrap_or(true)
                {
                    self.clear_mouse_selection();
                }
                Ok(true)
            }
            MouseEventKind::Down(MouseButton::Right) => {
                let text = self
                    .last_frame
                    .as_ref()
                    .map(|buffer| {
                        self.transcript_selection.as_ref().map_or_else(
                            || selected_screen_text(buffer, self.mouse_selection),
                            TranscriptSelection::text,
                        )
                    })
                    .unwrap_or_default();
                if text.trim().is_empty() {
                    return Ok(false);
                }
                if text.len() > COPY_LIMIT {
                    state.set_notice("Selection exceeds the 100 KiB copy limit");
                    return Ok(true);
                }
                let character_count = text.chars().count();
                match self.clipboard.copy(&text, &mut io::stdout()) {
                    Ok(CopyMethod::Native) => state
                        .set_notice(format!("Copied {character_count} chars to host clipboard")),
                    Ok(CopyMethod::Osc52) => {
                        state.set_notice(format!("Copied {character_count} chars with OSC52"))
                    }
                    Err(error) => state.set_notice(format!("Could not copy selection: {error}")),
                }
                self.clear_mouse_selection();
                Ok(true)
            }
            MouseEventKind::ScrollUp | MouseEventKind::ScrollDown => {
                self.clear_mouse_selection();
                Ok(false)
            }
            _ => Ok(false),
        }
    }

    fn clear_mouse_selection(&mut self) {
        self.mouse_selection.clear();
        self.transcript_selection = None;
    }

    fn scroll_mouse_selection(&mut self, state: &mut ChatViewState) -> bool {
        if self.selection_scroll_at.elapsed() < std::time::Duration::from_millis(50) {
            return false;
        }
        self.selection_scroll_at = std::time::Instant::now();
        self.transcript_selection
            .as_mut()
            .is_some_and(|selection| selection.scroll(&mut state.scroll_from_bottom))
    }

    fn composer_selection_range(&self, state: &ChatViewState) -> Option<(usize, usize)> {
        if self.transcript_selection.is_some() {
            return None;
        }
        let area = self.composer_area?;
        let (anchor, head) = self.mouse_selection.endpoints()?;
        let (start, end) = if anchor <= head {
            (anchor, head)
        } else {
            (head, anchor)
        };
        let chars = &state.composer;
        let row_width = (area.width as usize).saturating_sub(4).max(1);
        let start = input_index_at(start, area, chars, row_width, state.cursor, false)?;
        let end = input_index_at(end, area, chars, row_width, state.cursor, true)?;
        (start < end).then_some((start.min(chars.len()), end.min(chars.len())))
    }

    fn copy_mouse_selection(&mut self, state: &mut ChatViewState) -> bool {
        if self.mouse_selection.range().is_none() {
            return false;
        }
        let text = if let Some((start, end)) = self.composer_selection_range(state) {
            state.composer[start..end].iter().collect::<String>()
        } else {
            self.last_frame
                .as_ref()
                .map(|buffer| {
                    self.transcript_selection.as_ref().map_or_else(
                        || selected_screen_text(buffer, self.mouse_selection),
                        TranscriptSelection::text,
                    )
                })
                .unwrap_or_default()
        };
        if text.trim().is_empty() {
            self.clear_mouse_selection();
            state.set_notice("Nothing to copy from this selection");
            return true;
        }
        if text.len() > COPY_LIMIT {
            state.set_notice("Selection exceeds the 100 KiB copy limit");
        } else {
            let character_count = text.chars().count();
            match self.clipboard.copy(&text, &mut io::stdout()) {
                Ok(CopyMethod::Native | CopyMethod::Osc52) => {
                    state.set_notice(format!("Copied {character_count} chars to clipboard"))
                }
                Err(error) => state.set_notice(format!("Could not copy selection: {error}")),
            }
        }
        true
    }

    fn handle_tool_viewer_key(&mut self, state: &mut ChatViewState, key: event::KeyEvent) -> bool {
        if key.code == event::KeyCode::Char('y')
            && key.modifiers.contains(event::KeyModifiers::CONTROL)
            && let Some(tool) = state.tool_viewer.and_then(|index| state.tools.get(index))
        {
            match self.clipboard.copy(&tool.details(), &mut io::stdout()) {
                Ok(_) => state.set_notice("Tool details copied to clipboard"),
                Err(error) => state.set_notice(format!("Could not copy tool details: {error}")),
            }
            return true;
        }
        if state.handle_tool_key(key) {
            return true;
        }
        if state.tool_viewer.is_some() {
            if let Some(dialog) = state.dialog.as_mut()
                && dialog.handle_key(key).is_some()
            {
                state.dialog = None;
                state.tool_viewer = None;
            }
            return true;
        }
        false
    }

    pub fn read_input(
        &mut self,
        shared_state: &Arc<Mutex<ChatViewState>>,
    ) -> Result<Option<InteractiveInput>> {
        let mut state = shared_state
            .lock()
            .map_err(|_| anyhow::anyhow!("TUI state lock poisoned"))?;
        let mut pasted_image: Option<String> = None;
        let mut last_ctrl_d: Option<std::time::Instant> = None;
        let mut last_sync = std::time::Instant::now();
        state.mods = mint_core::mods::list(
            &state.current_dir,
            &mint_core::scoped_chat_id(&state.chat_id, Some(&state.current_dir.to_string_lossy())),
        )
        .unwrap_or_default();
        let mut previous_bg_count = mint_core::bg_shell::running_count_for(&state.current_dir);
        let mut redraw = true;
        let mut painted_notice_visible = false;
        let mut reported_mod_viewport = None;
        let mut watermark_started: Option<std::time::Instant> = None;
        const DOUBLE_PRESS_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(3);

        loop {
            let size = self.terminal.size()?;
            let viewport = state.mod_viewport(Rect::new(0, 0, size.width, size.height));
            if reported_mod_viewport.as_ref() != Some(&viewport) {
                let chat = mint_core::scoped_chat_id(
                    &state.chat_id,
                    Some(&state.current_dir.to_string_lossy()),
                );
                let _ =
                    mint_core::mods::update_viewport(&state.current_dir, &chat, viewport.clone());
                state.mods = mint_core::mods::list(&state.current_dir, &chat).unwrap_or_default();
                reported_mod_viewport = Some(viewport);
                redraw = true;
            }
            let watermark_visible = state.transcript.is_empty()
                && state.dialog.is_none()
                && state.thought_modal.is_none();
            if state.watermark_animating && watermark_visible {
                let started = *watermark_started.get_or_insert_with(std::time::Instant::now);
                match watermark_animation_at(started.elapsed()).0 {
                    Some(frame) if frame != state.watermark_frame => {
                        state.watermark_frame = frame;
                        redraw = true;
                    }
                    None => {
                        state.watermark_frame = 0;
                        state.watermark_animating = false;
                        watermark_started = None;
                        redraw = true;
                    }
                    _ => {}
                }
            } else {
                watermark_started = None;
            }

            if let Some(time) = last_ctrl_d {
                if time.elapsed() >= DOUBLE_PRESS_TIMEOUT {
                    last_ctrl_d = None;
                    if state.active_notice() == Some("Press Ctrl+D again to exit") {
                        state.clear_notice();
                    }
                }
            }

            let bg_count = mint_core::bg_shell::running_count_for(&state.current_dir);
            if bg_count != previous_bg_count {
                previous_bg_count = bg_count;
                redraw = true;
            }
            for notice in mint_core::bg_shell::take_finished_notices() {
                state.push_notice(notice);
                redraw = true;
            }
            redraw |= state.notice_visibility_changed_since_draw(painted_notice_visible);

            redraw |= self.scroll_mouse_selection(&mut state);
            if redraw {
                let notice_visible = state.active_notice().is_some();
                self.draw_state(&state)?;
                painted_notice_visible = notice_visible;
                redraw = false;
            }

            let watermark_animating = state.watermark_animating && watermark_visible;
            {
                let poll_interval = if watermark_animating {
                    let elapsed = watermark_started
                        .expect("active watermark has a start time")
                        .elapsed();
                    watermark_animation_at(elapsed)
                        .1
                        .unwrap_or(elapsed)
                        .saturating_sub(elapsed)
                        .min(std::time::Duration::from_millis(250))
                } else if self.mouse_selection.dragging
                    || last_ctrl_d.is_some()
                    || state.active_notice().is_some()
                {
                    std::time::Duration::from_millis(50)
                } else {
                    std::time::Duration::from_millis(250)
                };
                if !event::poll(poll_interval)? {
                    if last_sync.elapsed() >= std::time::Duration::from_millis(1500)
                        && self.mouse_selection.range().is_none()
                        && !state.selection_mode
                    {
                        redraw |= state.refresh_shared_transcript();
                        let mods = mint_core::mods::list(
                            &state.current_dir,
                            &mint_core::scoped_chat_id(
                                &state.chat_id,
                                Some(&state.current_dir.to_string_lossy()),
                            ),
                        )
                        .unwrap_or_default();
                        if mods != state.mods {
                            state.mods = mods;
                            redraw = true;
                        }
                        last_sync = std::time::Instant::now();
                    }
                    continue;
                }
            }

            let input_event = event::read()?;
            if matches!(
                &input_event,
                event::Event::Mouse(m) if m.kind == event::MouseEventKind::Moved
            ) {
                continue;
            }
            redraw = true;
            match input_event {
                event::Event::Resize(w, h) => {
                    self.clear_mouse_selection();
                    if w < MIN_WIDTH || h < MIN_HEIGHT {
                        state.set_notice(format!("Terminal too small: {w}x{h}"));
                    } else if state
                        .active_notice()
                        .map(|n| n.starts_with("Terminal too small"))
                        .unwrap_or(false)
                    {
                        state.clear_notice();
                    }
                }
                event::Event::Mouse(m) => {
                    if m.kind == event::MouseEventKind::Down(event::MouseButton::Left)
                        && self
                            .watermark_area
                            .is_some_and(|area| area.contains(Position::new(m.column, m.row)))
                    {
                        self.clear_mouse_selection();
                        state.watermark_frame = 0;
                        state.watermark_animating = true;
                        watermark_started = Some(std::time::Instant::now());
                        continue;
                    }
                    if self.handle_selection_mouse(m, &mut state)? {
                        continue;
                    }
                    route_mouse_wheel(&mut state, m.kind);
                }
                event::Event::Paste(text) => {
                    self.clear_mouse_selection();
                    state.insert_paste(&text);
                }
                event::Event::Key(key) if key.kind == event::KeyEventKind::Press => {
                    use event::{KeyCode, KeyModifiers};

                    if self.handle_tool_viewer_key(&mut state, key) {
                        continue;
                    }

                    let is_ctrl_c = key.code == KeyCode::Char('c')
                        && key.modifiers.contains(KeyModifiers::CONTROL);
                    if is_ctrl_c && self.copy_mouse_selection(&mut state) {
                        continue;
                    }

                    if self.mouse_selection.range().is_some() {
                        if matches!(key.code, KeyCode::Delete | KeyCode::Backspace) {
                            if let Some((start, end)) = self.composer_selection_range(&state) {
                                state.replace_composer(start..end, "");
                                state.clear_notice();
                            }
                            self.clear_mouse_selection();
                            continue;
                        }
                    }

                    self.clear_mouse_selection();

                    if key.code == KeyCode::F(7) && state.dialog.is_none() {
                        state.mods_open = !state.mods_open;
                        state.mod_input = None;
                        continue;
                    }
                    if state.mods_open && state.dialog.is_none() && state.thought_modal.is_none() {
                        let controls = crate::mods_ui::controls(&state.mods);
                        match key.code {
                            KeyCode::Esc => state.mods_open = false,
                            KeyCode::Tab => {
                                state.mod_focus = (state.mod_focus + 1) % controls.len().max(1);
                                state.mod_input = None;
                            }
                            KeyCode::BackTab => {
                                state.mod_focus = (state.mod_focus + controls.len().max(1) - 1)
                                    % controls.len().max(1);
                                state.mod_input = None;
                            }
                            KeyCode::Enter => {
                                if let Some(control) =
                                    controls.get(state.mod_focus % controls.len().max(1))
                                {
                                    let chat = mint_core::scoped_chat_id(
                                        &state.chat_id,
                                        Some(&state.current_dir.to_string_lossy()),
                                    );
                                    let value = if control.input {
                                        serde_json::json!(state.mod_input_value(control))
                                    } else {
                                        serde_json::Value::Null
                                    };
                                    let result = mint_core::mods::action(
                                        &state.current_dir,
                                        &chat,
                                        &control.id,
                                        &control.instance,
                                        &control.action,
                                        value,
                                    );
                                    if let Err(error) = result {
                                        state.set_notice(error);
                                    }
                                    state.mod_input = None;
                                    state.mods = mint_core::mods::list(&state.current_dir, &chat)
                                        .unwrap_or_default();
                                }
                            }
                            KeyCode::Char(c) if !key.modifiers.contains(KeyModifiers::CONTROL) => {
                                if let Some(control) = controls
                                    .get(state.mod_focus % controls.len().max(1))
                                    .filter(|c| c.input)
                                {
                                    state.mod_input_draft(control).push(c);
                                }
                            }
                            KeyCode::Backspace => {
                                if let Some(control) = controls
                                    .get(state.mod_focus % controls.len().max(1))
                                    .filter(|c| c.input)
                                {
                                    state.mod_input_draft(control).pop();
                                }
                            }
                            _ => {}
                        }
                        continue;
                    }
                    if key.code == KeyCode::F(6) {
                        let text = state.composer_text();
                        return Ok(Some(InteractiveInput {
                            text,
                            pasted_image,
                            switch_mode: true,
                        }));
                    }

                    if state.handle_thought_key(key) {
                        continue;
                    }

                    let is_ctrl_d = key.code == KeyCode::Char('d')
                        && key.modifiers.contains(KeyModifiers::CONTROL);
                    if !is_ctrl_d && last_ctrl_d.is_some() {
                        last_ctrl_d = None;
                        if state.active_notice() == Some("Press Ctrl+D again to exit") {
                            state.clear_notice();
                        }
                    }

                    if state.selection_mode {
                        match key.code {
                            KeyCode::Esc => state.selection_mode = false,
                            KeyCode::Char('c') | KeyCode::Char('d')
                                if key.modifiers.contains(KeyModifiers::CONTROL) =>
                            {
                                state.selection_mode = false;
                            }
                            KeyCode::Enter | KeyCode::Char('y') => {
                                let text = state.selected_text();
                                if text.len() > COPY_LIMIT {
                                    state.set_notice("Transcript exceeds the 100 KiB copy limit");
                                } else {
                                    match self.clipboard.copy(&text, &mut io::stdout()) {
                                        Ok(CopyMethod::Native) => {
                                            state.set_notice("Transcript copied to clipboard")
                                        }
                                        Ok(CopyMethod::Osc52) => {
                                            state.set_notice("Transcript copied with OSC52")
                                        }
                                        Err(error) => state.set_notice(format!(
                                            "Could not copy transcript: {error}"
                                        )),
                                    }
                                    state.selection_mode = false;
                                }
                            }
                            KeyCode::PageUp | KeyCode::Up => {
                                state.selection_head = state.selection_head.saturating_sub(1);
                                state.scroll_from_bottom =
                                    state.scroll_from_bottom.saturating_add(1)
                            }
                            KeyCode::PageDown | KeyCode::Down => {
                                let last = state.plain_transcript_lines().len().saturating_sub(1);
                                state.selection_head = (state.selection_head + 1).min(last);
                                state.scroll_from_bottom =
                                    state.scroll_from_bottom.saturating_sub(1)
                            }
                            _ => {}
                        }
                        continue;
                    }
                    let composer_width =
                        state.composer_width(Rect::new(0, 0, size.width, size.height));
                    if state.edit_composer_key(key) {
                        continue;
                    }
                    match key.code {
                        KeyCode::Char('v') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                            if let Ok(Some(uri)) = crate::image::read_clipboard_image() {
                                if let Some(current) = pasted_image.as_mut() {
                                    current.push(' ');
                                    current.push_str(&uri);
                                } else {
                                    pasted_image = Some(uri);
                                }
                                let mut image_draft = state.composer.clone();
                                let cursor = state.cursor;
                                let mut end = cursor;
                                super::insert_image_placeholder(&mut image_draft, &mut end);
                                state.replace_composer(
                                    cursor..cursor,
                                    &image_draft[cursor..end].iter().collect::<String>(),
                                );
                                state.set_notice("Image attached");
                            } else {
                                state.set_notice("No image found in clipboard");
                            }
                        }
                        KeyCode::PageUp => {
                            state.scroll_from_bottom = state.scroll_from_bottom.saturating_add(10)
                        }
                        KeyCode::PageDown => {
                            state.scroll_from_bottom = state.scroll_from_bottom.saturating_sub(10)
                        }
                        KeyCode::Home if key.modifiers.contains(KeyModifiers::CONTROL) => {
                            state.scroll_from_bottom = usize::MAX
                        }
                        KeyCode::Up if !state.suggestions().is_empty() => {
                            state.slash_selected = state.slash_selected.saturating_sub(1)
                        }
                        KeyCode::Down if !state.suggestions().is_empty() => {
                            let last = state.suggestions().len().saturating_sub(1);
                            state.slash_selected = (state.slash_selected + 1).min(last);
                        }
                        KeyCode::Tab if !state.suggestions().is_empty() => {
                            state.apply_selected_suggestion();
                        }
                        KeyCode::Up | KeyCode::Down | KeyCode::Home | KeyCode::End
                            if state.navigate_composer(key, composer_width) => {}
                        KeyCode::End => state.scroll_from_bottom = 0,
                        KeyCode::Up if !state.composer.contains(&'\n') => state.history_previous(),
                        KeyCode::Down if !state.composer.contains(&'\n') => state.history_next(),
                        KeyCode::Enter => {
                            let suggestions = state.suggestions();
                            let input: String = state.composer.iter().collect();
                            if input.starts_with('/')
                                && !suggestions.is_empty()
                                && !suggestions.iter().any(|(token, _)| token == input.trim())
                            {
                                state.apply_selected_suggestion();
                            }
                            let text = state.take_prompt();
                            state.history_index = None;
                            state.draft_before_history.clear();
                            state.clear_notice();
                            if !text.trim().is_empty() {
                                return Ok(Some(InteractiveInput {
                                    text,
                                    pasted_image,
                                    switch_mode: false,
                                }));
                            }
                        }
                        KeyCode::Char('d') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                            if let Some(time) = last_ctrl_d {
                                if time.elapsed() < DOUBLE_PRESS_TIMEOUT {
                                    state.stop_mods();
                                    return Ok(None);
                                }
                            }
                            last_ctrl_d = Some(std::time::Instant::now());
                            state.set_notice("Press Ctrl+D again to exit");
                        }
                        KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                            if !state.composer.is_empty() {
                                state.take_prompt();
                                state.slash_selected = 0;
                                state.clear_notice();
                            } else {
                                state.set_notice("Press Ctrl+D twice to exit");
                            }
                        }
                        _ => {}
                    }
                }
                _ => {}
            }
        }
    }
    pub fn handle(state: Arc<Mutex<ChatViewState>>) -> TuiHandle {
        TuiHandle {
            state,
            interrupted: Arc::new(AtomicBool::new(false)),
            queued: Arc::new(Mutex::new(Vec::new())),
        }
    }

    fn handle_dialog_event(
        &mut self,
        input_event: event::Event,
        state: &mut ChatViewState,
    ) -> Result<()> {
        match input_event {
            event::Event::Mouse(mouse) => {
                if !self.handle_selection_mouse(mouse, state)? {
                    route_mouse_wheel(state, mouse.kind);
                }
            }
            event::Event::Resize(_, _) => self.clear_mouse_selection(),
            event::Event::Key(key) if key.kind == event::KeyEventKind::Press => {
                self.clear_mouse_selection();
                if let Some(dialog) = state.dialog.as_mut()
                    && let Some(answer) = dialog.handle_key(key)
                    && let Some(dialog) = state.dialog.take()
                {
                    let _ = dialog.reply.send(answer);
                }
            }
            _ => {}
        }
        Ok(())
    }

    pub fn prompt_choice(
        &mut self,
        handle: &TuiHandle,
        title: impl Into<String>,
        body: impl Into<String>,
        options: Vec<String>,
    ) -> Result<Option<usize>> {
        let (reply, response) = mpsc::channel();
        if let Ok(mut state) = handle.state.lock() {
            state.dialog = Some(DialogState::new_choice(title, body, options, None, reply));
        }
        self.wait_for_choice(handle, response)
    }

    pub fn prompt_update(
        &mut self,
        handle: &TuiHandle,
        current: &str,
        latest: &str,
    ) -> Result<Option<usize>> {
        let (reply, response) = mpsc::channel();
        if let Ok(mut state) = handle.state.lock() {
            state.dialog = Some(DialogState::new_update(current, latest, reply));
        }
        self.wait_for_choice(handle, response)
    }

    fn wait_for_dialog_answer(
        &mut self,
        handle: &TuiHandle,
        response: mpsc::Receiver<DialogAnswer>,
    ) -> Result<DialogAnswer> {
        loop {
            if let Ok(mut state) = handle.state.lock() {
                self.scroll_mouse_selection(&mut state);
                self.draw_state(&state)?;
            }
            if let Ok(answer) = response.try_recv() {
                return Ok(answer);
            }
            if event::poll(std::time::Duration::from_millis(50))?
                && let Ok(mut state) = handle.state.lock()
            {
                self.handle_dialog_event(event::read()?, &mut state)?;
            }
        }
    }

    fn wait_for_choice(
        &mut self,
        handle: &TuiHandle,
        response: mpsc::Receiver<DialogAnswer>,
    ) -> Result<Option<usize>> {
        Ok(match self.wait_for_dialog_answer(handle, response)? {
            DialogAnswer::Choice(index) => Some(index),
            _ => None,
        })
    }

    pub fn prompt_theme(&mut self, handle: &TuiHandle, current: usize) -> Result<Option<usize>> {
        let (reply, response) = mpsc::channel();
        if let Ok(mut state) = handle.state.lock() {
            state.dialog = Some(DialogState::new_theme_picker(current, reply));
        }
        self.wait_for_choice(handle, response)
    }

    pub fn prompt_resume_picker(
        &mut self,
        handle: &TuiHandle,
        sessions: Vec<mint_core::ChatSession>,
        current_dir: &std::path::Path,
        current_branch: Option<String>,
        active_chat_id: &str,
    ) -> Result<Option<usize>> {
        let (reply, response) = mpsc::channel();
        if let Ok(mut state) = handle.state.lock() {
            state.dialog = Some(DialogState::new_resume_picker(
                sessions,
                current_dir.to_string_lossy().to_string(),
                current_branch,
                active_chat_id.to_string(),
                reply,
            ));
        }
        self.wait_for_choice(handle, response)
    }

    #[allow(dead_code)]
    pub fn prompt_multi_choice(
        &mut self,
        handle: &TuiHandle,
        title: impl Into<String>,
        body: impl Into<String>,
        options: Vec<String>,
    ) -> Result<Option<Vec<usize>>> {
        let (reply, response) = mpsc::channel();
        let checked = vec![false; options.len()];
        if let Ok(mut state) = handle.state.lock() {
            state.dialog = Some(DialogState::new_choice(
                title,
                body,
                options,
                Some(checked),
                reply,
            ));
        }
        Ok(match self.wait_for_dialog_answer(handle, response)? {
            DialogAnswer::Choices(indices) => Some(indices),
            _ => None,
        })
    }
    pub fn prompt_text(
        &mut self,
        handle: &TuiHandle,
        title: impl Into<String>,
        body: impl Into<String>,
        initial: Option<&str>,
    ) -> Result<Option<String>> {
        let (reply, response) = mpsc::channel();
        if let Ok(mut state) = handle.state.lock() {
            let mut dialog = DialogState::new_text(title, body, reply);
            if let Some(init) = initial {
                dialog.input = Some(init.chars().collect());
            }
            state.dialog = Some(dialog);
        }
        Ok(match self.wait_for_dialog_answer(handle, response)? {
            DialogAnswer::Text(text) if !text.trim().is_empty() => Some(text),
            _ => None,
        })
    }
    pub fn run_onboard_wizard(&mut self, config: &mut mint_core::MintConfig) -> Result<bool> {
        let mut wizard = crate::onboard::OnboardWizard::new(config.clone());
        let outcome = wizard.run_with_terminal(&mut self.terminal)?;
        match outcome {
            crate::onboard::OnboardOutcome::SavedAndLaunch
            | crate::onboard::OnboardOutcome::SavedAndExit => {
                *config = wizard.take_config();
                Ok(true)
            }
            crate::onboard::OnboardOutcome::Cancelled => Ok(false),
        }
    }
    pub async fn drive_agent<T: Send + 'static>(
        &mut self,
        handle: &TuiHandle,
        task: tokio::task::JoinHandle<T>,
    ) -> Result<T> {
        use event::{KeyCode, KeyModifiers};
        loop {
            if let Ok(mut state) = handle.state.lock() {
                if let Ok(queued) = handle.queued.lock() {
                    state.queued_count = queued.len();
                    state.queued_preview = queued.last().cloned();
                }
                self.scroll_mouse_selection(&mut state);
                self.draw_state(&state)?;
            }
            if task.is_finished() {
                if let Ok(mut state) = handle.state.lock() {
                    state.finish_live_thought();
                    state.finish_running_tools();
                    state.queued_count = 0;
                    state.queued_preview = None;
                }
                return Ok(task.await?);
            }
            if event::poll(std::time::Duration::from_millis(30))? {
                match event::read()? {
                    event::Event::Resize(_, _) => self.clear_mouse_selection(),
                    event::Event::Mouse(mouse) => {
                        if let Ok(mut state) = handle.state.lock() {
                            if self.handle_selection_mouse(mouse, &mut state)? {
                                continue;
                            }
                            route_mouse_wheel(&mut state, mouse.kind);
                        }
                    }
                    event::Event::Paste(text) => {
                        self.clear_mouse_selection();
                        if let Ok(mut state) = handle.state.lock() {
                            state.insert_paste(&text);
                        }
                    }
                    event::Event::Key(key) if key.kind == event::KeyEventKind::Press => {
                        let mut selection_handled = false;
                        if let Ok(mut state) = handle.state.lock() {
                            if self.handle_tool_viewer_key(&mut state, key) {
                                selection_handled = true;
                            } else if let Some(dialog) = state.dialog.as_mut() {
                                if let Some(answer) = dialog.handle_key(key) {
                                    if let Some(dialog) = state.dialog.take() {
                                        let _ = dialog.reply.send(answer);
                                    }
                                }
                                selection_handled = true;
                            } else if state.handle_thought_key(key) {
                                selection_handled = true;
                            } else if key.code == KeyCode::Char('c')
                                && key.modifiers.contains(KeyModifiers::CONTROL)
                                && self.copy_mouse_selection(&mut state)
                            {
                                selection_handled = true;
                            } else if self.mouse_selection.range().is_some()
                                && matches!(key.code, KeyCode::Delete | KeyCode::Backspace)
                            {
                                if let Some((start, end)) = self.composer_selection_range(&state) {
                                    state.replace_composer(start..end, "");
                                }
                                self.clear_mouse_selection();
                                selection_handled = true;
                            }
                        }
                        if selection_handled {
                            continue;
                        }
                        self.clear_mouse_selection();
                        if key.code == KeyCode::Esc
                            || (key.code == KeyCode::Char('c')
                                && key.modifiers.contains(KeyModifiers::CONTROL))
                        {
                            handle.interrupted.store(true, Ordering::Relaxed);
                            continue;
                        }
                        if let Ok(mut state) = handle.state.lock() {
                            let size = self.terminal.size()?;
                            let composer_width =
                                state.composer_width(Rect::new(0, 0, size.width, size.height));
                            if state.edit_composer_key(key) {
                                continue;
                            }
                            match key.code {
                                KeyCode::Char('d')
                                    if key.modifiers.contains(KeyModifiers::CONTROL) =>
                                {
                                    if let Ok(mut queued) = handle.queued.lock() {
                                        queued.pop();
                                        state.queued_count = queued.len();
                                        state.queued_preview = queued.last().cloned();
                                    }
                                }
                                KeyCode::Up | KeyCode::Down | KeyCode::Home | KeyCode::End
                                    if state.navigate_composer(key, composer_width) => {}
                                KeyCode::End => state.scroll_from_bottom = 0,
                                KeyCode::Enter => {
                                    if !state.composer.is_empty() {
                                        let text = state.take_prompt();
                                        if let Ok(mut queued) = handle.queued.lock() {
                                            queued.push(text);
                                            state.queued_count = queued.len();
                                            state.queued_preview = queued.last().cloned();
                                        }
                                    }
                                }
                                _ => {}
                            }
                        }
                    }
                    _ => {}
                }
            }
            tokio::task::yield_now().await;
        }
    }
    pub fn suspend(&mut self) {
        if !self.active {
            return;
        }
        let _ = self.terminal.show_cursor();
        restore_terminal();
        self.active = false;
    }
    pub fn resume(&mut self) -> Result<()> {
        if self.active {
            return Ok(());
        }
        terminal::enable_raw_mode()?;
        let mut stdout = io::stdout();
        let setup = setup_terminal_output(&mut stdout);
        if let Err(error) = setup {
            restore_terminal();
            return Err(error.into());
        }
        if let Err(error) = self.terminal.clear() {
            restore_terminal();
            return Err(error.into());
        }
        self.active = true;
        Ok(())
    }
}
impl Drop for ChatTui {
    fn drop(&mut self) {
        self.suspend();
    }
}

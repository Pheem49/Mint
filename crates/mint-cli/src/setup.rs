use anyhow::Result;
use crossterm::ExecutableCommand;
use crossterm::cursor::{Hide, Show};
use crossterm::event::{self, Event, KeyCode, KeyModifiers};
use crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};
use ratatui::{
    Terminal,
    backend::{Backend, CrosstermBackend},
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, List, ListItem, ListState, Paragraph},
};
use std::collections::HashSet;
use std::time::Duration;
use unicode_width::UnicodeWidthStr;

use crate::terminal_theme::{
    ACCENT, BLUE, MUTED, SELECTION_BACKGROUND, SELECTION_TEXT, SUBTLE, TEXT,
};
use mint_core::{MintConfig, load_config, native_plugins, save_config};

// ─────────────────────────────────────────────────────────────────────────────
// Types & Data Structures
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SetupStep {
    Tools,
    Plugins,
    Launch,
}

impl SetupStep {
    pub fn index(&self) -> usize {
        match self {
            Self::Tools => 0,
            Self::Plugins => 1,
            Self::Launch => 2,
        }
    }
}

#[derive(Debug, Clone)]
pub struct ToolItem {
    pub category: &'static str,
    pub name: &'static str,
    pub key: &'static str,
    pub description: &'static str,
    pub enabled: bool,
}

#[derive(Debug, Clone)]
pub struct PluginItem {
    pub name: &'static str,
    pub description: &'static str,
    pub enabled: bool,
}

#[derive(Debug, Clone)]
pub struct LaunchTargetItem {
    pub title: &'static str,
    pub key: Option<&'static str>,
    pub description: &'static str,
}

// ─────────────────────────────────────────────────────────────────────────────
// Setup Wizard State & Engine
// ─────────────────────────────────────────────────────────────────────────────

pub struct SetupWizard {
    pub config: MintConfig,
    pub step: SetupStep,

    // Step 1: Agent Tools
    pub tools: Vec<ToolItem>,
    pub tool_cursor: usize,

    // Step 2: Native Plugins
    pub plugins: Vec<PluginItem>,
    pub plugin_cursor: usize,

    // Step 3: Launch Target
    pub launch_options: Vec<LaunchTargetItem>,
    pub launch_cursor: usize,
}

impl SetupWizard {
    pub fn new(config: MintConfig) -> Self {
        let tools = vec![
            // 📁 Files & Workspace
            ToolItem {
                category: "Files & Workspace",
                name: "list_files",
                key: "list_files",
                description: "List files and directories in the workspace",
                enabled: !config.disabled_tools.contains(&"list_files".to_string()),
            },
            ToolItem {
                category: "Files & Workspace",
                name: "read_file",
                key: "read_file",
                description: "Read file contents (text, code, documents)",
                enabled: !config.disabled_tools.contains(&"read_file".to_string()),
            },
            ToolItem {
                category: "Files & Workspace",
                name: "write_file",
                key: "write_file",
                description: "Create new files or overwrite existing files",
                enabled: !config.disabled_tools.contains(&"write_file".to_string()),
            },
            ToolItem {
                category: "Files & Workspace",
                name: "apply_patch",
                key: "apply_patch",
                description: "Apply surgical edits and patch diffs to files",
                enabled: !config.disabled_tools.contains(&"apply_patch".to_string()),
            },
            // 💻 Code & Repository
            ToolItem {
                category: "Code & Repository",
                name: "search_code",
                key: "search_code",
                description: "Search workspace code text using regex/patterns",
                enabled: !config.disabled_tools.contains(&"search_code".to_string()),
            },
            ToolItem {
                category: "Code & Repository",
                name: "symbols",
                key: "symbols",
                description: "Index and search code symbols and definitions",
                enabled: !config.disabled_tools.contains(&"symbols".to_string()),
            },
            ToolItem {
                category: "Code & Repository",
                name: "git_status",
                key: "git_status",
                description: "Read git repository status and branch state",
                enabled: !config.disabled_tools.contains(&"git_status".to_string()),
            },
            ToolItem {
                category: "Code & Repository",
                name: "git_diff",
                key: "git_diff",
                description: "Inspect working tree and commit diffs",
                enabled: !config.disabled_tools.contains(&"git_diff".to_string()),
            },
            ToolItem {
                category: "Code & Repository",
                name: "git_log",
                key: "git_log",
                description: "Read git commit history logs",
                enabled: !config.disabled_tools.contains(&"git_log".to_string()),
            },
            ToolItem {
                category: "Code & Repository",
                name: "git_branch",
                key: "git_branch",
                description: "List, inspect, and switch git branches",
                enabled: !config.disabled_tools.contains(&"git_branch".to_string()),
            },
            ToolItem {
                category: "Code & Repository",
                name: "detect_project",
                key: "detect_project",
                description: "Detect project type, stack, and workspace manifest",
                enabled: !config
                    .disabled_tools
                    .contains(&"detect_project".to_string()),
            },
            ToolItem {
                category: "Code & Repository",
                name: "list_tests",
                key: "list_tests",
                description: "Discover and enumerate automated test suites",
                enabled: !config.disabled_tools.contains(&"list_tests".to_string()),
            },
            ToolItem {
                category: "Code & Repository",
                name: "read_diagnostics",
                key: "read_diagnostics",
                description: "Read compiler diagnostics and linter errors",
                enabled: !config
                    .disabled_tools
                    .contains(&"read_diagnostics".to_string()),
            },
            // 🔍 Search & Knowledge
            ToolItem {
                category: "Search & Knowledge",
                name: "web_search",
                key: "web_search",
                description: "Search the web for up-to-date documentation & answers",
                enabled: !config.disabled_tools.contains(&"web_search".to_string()),
            },
            ToolItem {
                category: "Search & Knowledge",
                name: "image_search",
                key: "image_search",
                description: "Search for images on the web",
                enabled: !config.disabled_tools.contains(&"image_search".to_string()),
            },
            ToolItem {
                category: "Search & Knowledge",
                name: "semantic_search",
                key: "semantic_search",
                description: "Semantic natural language search over indexed code",
                enabled: !config
                    .disabled_tools
                    .contains(&"semantic_search".to_string()),
            },
            ToolItem {
                category: "Search & Knowledge",
                name: "semantic_index",
                key: "semantic_index",
                description: "Build local vector embeddings for semantic search",
                enabled: !config
                    .disabled_tools
                    .contains(&"semantic_index".to_string()),
            },
            ToolItem {
                category: "Search & Knowledge",
                name: "knowledge_search",
                key: "knowledge_search",
                description: "Search local knowledge base documents and notes",
                enabled: !config
                    .disabled_tools
                    .contains(&"knowledge_search".to_string()),
            },
            // 🧠 Productivity & Utilities
            ToolItem {
                category: "Productivity & Utilities",
                name: "weather",
                key: "weather",
                description: "Check real-time weather conditions and forecasts",
                enabled: !config.disabled_tools.contains(&"weather".to_string()),
            },
            ToolItem {
                category: "Productivity & Utilities",
                name: "stock",
                key: "stock",
                description: "Query real-time stock and market prices",
                enabled: !config.disabled_tools.contains(&"stock".to_string()),
            },
            ToolItem {
                category: "Productivity & Utilities",
                name: "calculation",
                key: "calculation",
                description: "Perform math, currency, and unit calculations",
                enabled: !config.disabled_tools.contains(&"calculation".to_string()),
            },
            ToolItem {
                category: "Productivity & Utilities",
                name: "memory_recall",
                key: "memory_recall",
                description: "Recall persistent long-term memories across sessions",
                enabled: !config.disabled_tools.contains(&"memory_recall".to_string()),
            },
            ToolItem {
                category: "Productivity & Utilities",
                name: "note_write",
                key: "note_write",
                description: "Write and save notes to linked memory folders",
                enabled: !config.disabled_tools.contains(&"note_write".to_string()),
            },
            // 🛡️ Planning & Safety
            ToolItem {
                category: "Planning & Safety",
                name: "create_plan",
                key: "create_plan",
                description: "Create structured multi-step task plans and milestones",
                enabled: !config.disabled_tools.contains(&"create_plan".to_string()),
            },
            ToolItem {
                category: "Planning & Safety",
                name: "update_plan",
                key: "update_plan",
                description: "Update task plan progress and step completion",
                enabled: !config.disabled_tools.contains(&"update_plan".to_string()),
            },
            ToolItem {
                category: "Planning & Safety",
                name: "request_user_approval",
                key: "request_user_approval",
                description: "Request explicit user approval for risky actions",
                enabled: !config
                    .disabled_tools
                    .contains(&"request_user_approval".to_string()),
            },
            ToolItem {
                category: "Planning & Safety",
                name: "ask_user",
                key: "ask_user",
                description: "Ask the user interactive clarifying questions",
                enabled: !config.disabled_tools.contains(&"ask_user".to_string()),
            },
            ToolItem {
                category: "Planning & Safety",
                name: "verify",
                key: "verify",
                description: "Run verification checks and automated assertions",
                enabled: !config.disabled_tools.contains(&"verify".to_string()),
            },
            // ⚙️ Execution & MCP
            ToolItem {
                category: "Execution & MCP",
                name: "run_shell",
                key: "run_shell",
                description: "Execute terminal commands under safety policy checks",
                enabled: !config.disabled_tools.contains(&"run_shell".to_string()),
            },
            ToolItem {
                category: "Execution & MCP",
                name: "run_plugin",
                key: "run_plugin",
                description: "Execute native Mint plugin operations",
                enabled: !config.disabled_tools.contains(&"run_plugin".to_string()),
            },
            ToolItem {
                category: "Execution & MCP",
                name: "mcp_tool",
                key: "mcp_tool",
                description: "Invoke configured Model Context Protocol (MCP) tools",
                enabled: !config.disabled_tools.contains(&"mcp_tool".to_string()),
            },
            ToolItem {
                category: "Execution & MCP",
                name: "mcp_list_tools",
                key: "mcp_list_tools",
                description: "Discover available MCP server tools",
                enabled: !config
                    .disabled_tools
                    .contains(&"mcp_list_tools".to_string()),
            },
            // 🎬 Media & Audio/Video
            ToolItem {
                category: "Media & Audio/Video",
                name: "view_image",
                key: "view_image",
                description: "Inspect and analyze image attachments with deduplication",
                enabled: !config.disabled_tools.contains(&"view_image".to_string()),
            },
            ToolItem {
                category: "Media & Audio/Video",
                name: "generate_image",
                key: "generate_image",
                description: "Generate images via NanoBanana (Google Gemini)",
                enabled: !config
                    .disabled_tools
                    .contains(&"generate_image".to_string()),
            },
            ToolItem {
                category: "Media & Audio/Video",
                name: "generate_video",
                key: "generate_video",
                description: "Generate short videos via Google Veo",
                enabled: !config
                    .disabled_tools
                    .contains(&"generate_video".to_string()),
            },
            ToolItem {
                category: "Media & Audio/Video",
                name: "video_trim",
                key: "video_trim",
                description: "Trim video duration by precise timestamps",
                enabled: !config.disabled_tools.contains(&"video_trim".to_string()),
            },
            ToolItem {
                category: "Media & Audio/Video",
                name: "video_remove_silence",
                key: "video_remove_silence",
                description: "Automatically remove silence and pauses from video",
                enabled: !config
                    .disabled_tools
                    .contains(&"video_remove_silence".to_string()),
            },
            ToolItem {
                category: "Media & Audio/Video",
                name: "video_resize",
                key: "video_resize",
                description: "Resize video resolution and aspect ratios",
                enabled: !config.disabled_tools.contains(&"video_resize".to_string()),
            },
            ToolItem {
                category: "Media & Audio/Video",
                name: "video_merge",
                key: "video_merge",
                description: "Concatenate and merge multiple video clips",
                enabled: !config.disabled_tools.contains(&"video_merge".to_string()),
            },
            ToolItem {
                category: "Media & Audio/Video",
                name: "video_export",
                key: "video_export",
                description: "Render and export video files with encoding presets",
                enabled: !config.disabled_tools.contains(&"video_export".to_string()),
            },
            ToolItem {
                category: "Media & Audio/Video",
                name: "video_extract_audio",
                key: "video_extract_audio",
                description: "Extract audio tracks (MP3/WAV) from video files",
                enabled: !config
                    .disabled_tools
                    .contains(&"video_extract_audio".to_string()),
            },
            ToolItem {
                category: "Media & Audio/Video",
                name: "speech_transcribe",
                key: "speech_transcribe",
                description: "Transcribe voice audio to timestamped text",
                enabled: !config
                    .disabled_tools
                    .contains(&"speech_transcribe".to_string()),
            },
            ToolItem {
                category: "Media & Audio/Video",
                name: "subtitle_generate",
                key: "subtitle_generate",
                description: "Auto-generate SRT and VTT subtitle files",
                enabled: !config
                    .disabled_tools
                    .contains(&"subtitle_generate".to_string()),
            },
            ToolItem {
                category: "Media & Audio/Video",
                name: "subtitle_translate",
                key: "subtitle_translate",
                description: "Translate subtitles across multiple languages",
                enabled: !config
                    .disabled_tools
                    .contains(&"subtitle_translate".to_string()),
            },
            ToolItem {
                category: "Media & Audio/Video",
                name: "subtitle_burn",
                key: "subtitle_burn",
                description: "Burn hard subtitles directly into video streams",
                enabled: !config.disabled_tools.contains(&"subtitle_burn".to_string()),
            },
            ToolItem {
                category: "Media & Audio/Video",
                name: "timeline_reorder",
                key: "timeline_reorder",
                description: "Reorder and arrange clips on the video timeline",
                enabled: !config
                    .disabled_tools
                    .contains(&"timeline_reorder".to_string()),
            },
            ToolItem {
                category: "Media & Audio/Video",
                name: "effect_zoom_on_speaker",
                key: "effect_zoom_on_speaker",
                description: "Dynamic camera auto-zoom on active speaker",
                enabled: !config
                    .disabled_tools
                    .contains(&"effect_zoom_on_speaker".to_string()),
            },
            ToolItem {
                category: "Media & Audio/Video",
                name: "audio_duck_music",
                key: "audio_duck_music",
                description: "Duck background music volume behind voice tracks",
                enabled: !config
                    .disabled_tools
                    .contains(&"audio_duck_music".to_string()),
            },
            ToolItem {
                category: "Media & Audio/Video",
                name: "make_shorts",
                key: "make_shorts",
                description: "Generate vertical short clips (9:16) from video",
                enabled: !config.disabled_tools.contains(&"make_shorts".to_string()),
            },
        ];

        let all_native = native_plugins();
        let allowed_plugins: HashSet<String> = config
            .extra
            .get("allowedNativePlugins")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(|s| s.to_string()))
                    .collect()
            })
            .unwrap_or_else(|| {
                vec![
                    "dev_tools".to_string(),
                    "system_metrics".to_string(),
                    "github".to_string(),
                ]
                .into_iter()
                .collect()
            });

        let plugins = all_native
            .into_iter()
            .map(|p| {
                let has_all_permission = allowed_plugins.contains("*");
                let is_enabled = has_all_permission || allowed_plugins.contains(p.name);
                PluginItem {
                    name: p.name,
                    description: p.description,
                    enabled: is_enabled,
                }
            })
            .collect();

        let launch_options = vec![
            LaunchTargetItem {
                title: "1. CLI Interactive Chat Assistant",
                key: Some("cli"),
                description: "Launch the full-screen terminal interactive assistant immediately",
            },
            LaunchTargetItem {
                title: "2. Web Application UI",
                key: Some("web"),
                description: "Start Mint local API server and launch the Vite Web App in your browser",
            },
            LaunchTargetItem {
                title: "3. Desktop Application",
                key: Some("app_link"),
                description: "Open the Mint Desktop app link / download page in default browser",
            },
            LaunchTargetItem {
                title: "4. Save & Exit",
                key: None,
                description: "Save updated configuration and return to shell without launching",
            },
        ];

        Self {
            config,
            step: SetupStep::Tools,
            tools,
            tool_cursor: 0,
            plugins,
            plugin_cursor: 0,
            launch_options,
            launch_cursor: 0,
        }
    }

    // ─────────────────────────────────────────────────────────────────────────
    // Save Logic
    // ─────────────────────────────────────────────────────────────────────────

    pub fn save_changes(&mut self) -> Result<()> {
        let disabled: Vec<String> = self
            .tools
            .iter()
            .filter(|o| !o.enabled)
            .map(|o| o.key.to_string())
            .collect();
        self.config.disabled_tools = disabled;

        let enabled_natives: Vec<serde_json::Value> = self
            .plugins
            .iter()
            .filter(|o| o.enabled)
            .map(|o| serde_json::Value::String(o.name.to_string()))
            .collect();

        self.config.extra.insert(
            "allowedNativePlugins".to_string(),
            serde_json::Value::Array(enabled_natives),
        );

        save_config(&self.config)?;
        Ok(())
    }

    // ─────────────────────────────────────────────────────────────────────────
    // Render Functions
    // ─────────────────────────────────────────────────────────────────────────

    pub fn render(&mut self, frame: &mut ratatui::Frame) {
        let area = frame.area();
        let main_chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3), // Stepper header
                Constraint::Min(10),   // Content area
                Constraint::Length(1), // Footer status bar
            ])
            .split(area);

        self.render_stepper_header(frame, main_chunks[0]);

        match self.step {
            SetupStep::Tools => self.render_step_1_tools(frame, main_chunks[1]),
            SetupStep::Plugins => self.render_step_2_plugins(frame, main_chunks[1]),
            SetupStep::Launch => self.render_step_3_launch(frame, main_chunks[1]),
        }

        self.render_footer(frame, main_chunks[2]);
    }

    fn render_stepper_header(&self, frame: &mut ratatui::Frame, area: Rect) {
        let current_idx = self.step.index();
        let step_spans = vec![
            Self::step_badge("1. Agent Tools", current_idx == 0, current_idx > 0),
            Span::styled(" ── ", Style::default().fg(MUTED)),
            Self::step_badge("2. Native Plugins", current_idx == 1, current_idx > 1),
            Span::styled(" ── ", Style::default().fg(MUTED)),
            Self::step_badge("3. Launch Target", current_idx == 2, false),
        ];

        let block = Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(SUBTLE))
            .title(Span::styled(
                " Mint Tool & Environment Setup ",
                Style::default().fg(TEXT).add_modifier(Modifier::BOLD),
            ));

        let paragraph = Paragraph::new(Line::from(step_spans))
            .block(block)
            .alignment(Alignment::Center);

        frame.render_widget(paragraph, area);
    }

    fn step_badge(title: &str, is_current: bool, is_done: bool) -> Span<'static> {
        if is_current {
            Span::styled(
                format!(" [{title}] "),
                Style::default()
                    .fg(SELECTION_TEXT)
                    .bg(SELECTION_BACKGROUND)
                    .add_modifier(Modifier::BOLD),
            )
        } else if is_done {
            Span::styled(
                format!(" ✔ {title} "),
                Style::default().fg(TEXT).add_modifier(Modifier::BOLD),
            )
        } else {
            Span::styled(format!(" {title} "), Style::default().fg(MUTED))
        }
    }

    fn render_step_1_tools(&mut self, frame: &mut ratatui::Frame, area: Rect) {
        let block = Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(SUBTLE))
            .title(Span::styled(
                " Step 1: Agent Tools Manager ",
                Style::default().fg(TEXT).add_modifier(Modifier::BOLD),
            ));

        let inner = block.inner(area);
        frame.render_widget(block, area);

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .margin(1)
            .constraints([
                Constraint::Length(2), // Subtitle
                Constraint::Min(10),   // Tools list
                Constraint::Length(2), // Action hint
            ])
            .split(inner);

        let subtitle = Paragraph::new(
            "Select agent tools to enable or disable for your assistant.\nTools disabled here will not be called during autonomous reasoning sessions."
        ).style(Style::default().fg(MUTED));
        frame.render_widget(subtitle, chunks[0]);

        let mut list_items = Vec::new();
        let mut cur_category = "";
        let list_inner_width = chunks[1].width.saturating_sub(2) as usize;

        for (i, tool) in self.tools.iter().enumerate() {
            let is_cursor = i == self.tool_cursor;
            let check_icon = if tool.enabled { " ◉ " } else { " ○ " };
            let cursor_prefix = if is_cursor { " › " } else { "   " };

            let mut lines = Vec::new();
            if tool.category != cur_category {
                cur_category = tool.category;
                lines.push(Line::from(vec![Span::styled(
                    format!("  ── {} ──", tool.category),
                    Style::default().fg(BLUE).add_modifier(Modifier::BOLD),
                )]));
            }

            if is_cursor {
                let check_style = Style::default()
                    .fg(if tool.enabled {
                        Color::Black
                    } else {
                        Color::DarkGray
                    })
                    .bg(SELECTION_BACKGROUND)
                    .add_modifier(Modifier::BOLD);

                let desc_part = format!("  — {}", tool.description);
                let full_row_text = format!("{cursor_prefix}{check_icon}{}{desc_part}", tool.name);
                let visual_width = UnicodeWidthStr::width(full_row_text.as_str());
                let pad_len = list_inner_width.saturating_sub(visual_width);

                lines.push(Line::from(vec![
                    Span::styled(
                        cursor_prefix,
                        Style::default()
                            .fg(SELECTION_TEXT)
                            .bg(SELECTION_BACKGROUND)
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(check_icon, check_style),
                    Span::styled(
                        tool.name,
                        Style::default()
                            .fg(SELECTION_TEXT)
                            .bg(SELECTION_BACKGROUND)
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(
                        desc_part,
                        Style::default().fg(SELECTION_TEXT).bg(SELECTION_BACKGROUND),
                    ),
                    Span::styled(
                        " ".repeat(pad_len),
                        Style::default().bg(SELECTION_BACKGROUND),
                    ),
                ]));
                list_items.push(ListItem::new(lines));
            } else {
                let name_style = if tool.enabled {
                    Style::default().fg(TEXT)
                } else {
                    Style::default().fg(MUTED)
                };
                let check_color = if tool.enabled { ACCENT } else { MUTED };

                lines.push(Line::from(vec![
                    Span::raw(cursor_prefix),
                    Span::styled(
                        check_icon,
                        Style::default()
                            .fg(check_color)
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(tool.name, name_style),
                    Span::styled(
                        format!("  — {}", tool.description),
                        Style::default().fg(MUTED),
                    ),
                ]));
                list_items.push(ListItem::new(lines));
            }
        }

        let enabled_count = self.tools.iter().filter(|t| t.enabled).count();
        let total_count = self.tools.len();

        let list_block = Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(TEXT))
            .title(Line::from(vec![
                Span::styled(
                    " Agent Tools ",
                    Style::default().fg(TEXT).add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    " [ SELECTING ] ",
                    Style::default()
                        .fg(SELECTION_TEXT)
                        .bg(SELECTION_BACKGROUND)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    format!(" [Enabled: {enabled_count} / {total_count} tools] "),
                    Style::default().fg(TEXT),
                ),
                Span::styled(
                    " [Space: Toggle | a: All | i: Invert] ",
                    Style::default().fg(MUTED),
                ),
            ]));

        let mut list_state = ListState::default();
        list_state.select(Some(self.tool_cursor));
        frame.render_stateful_widget(
            List::new(list_items).block(list_block),
            chunks[1],
            &mut list_state,
        );

        let action_hint = Paragraph::new(Line::from(vec![
            Span::styled(" [ Esc: Cancel ] ", Style::default().fg(MUTED)),
            Span::raw("   "),
            Span::styled(
                " [ Enter: Next Step (Native Plugins) ➔ ] ",
                Style::default()
                    .fg(SELECTION_TEXT)
                    .bg(SELECTION_BACKGROUND)
                    .add_modifier(Modifier::BOLD),
            ),
        ]))
        .alignment(Alignment::Right);
        frame.render_widget(action_hint, chunks[2]);
    }

    fn render_step_2_plugins(&mut self, frame: &mut ratatui::Frame, area: Rect) {
        let block = Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(SUBTLE))
            .title(Span::styled(
                " Step 2: Native Plugins Access ",
                Style::default().fg(TEXT).add_modifier(Modifier::BOLD),
            ));

        let inner = block.inner(area);
        frame.render_widget(block, area);

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .margin(1)
            .constraints([
                Constraint::Length(2), // Subtitle
                Constraint::Min(8),    // Plugins list
                Constraint::Length(2), // Action hint
            ])
            .split(inner);

        let subtitle = Paragraph::new(
            "Select which native host plugins are allowed to run.\nPlugins provide extended local access like system diagnostics and developer tools."
        ).style(Style::default().fg(MUTED));
        frame.render_widget(subtitle, chunks[0]);

        let mut list_items = Vec::new();
        let list_inner_width = chunks[1].width.saturating_sub(2) as usize;

        for (i, plugin) in self.plugins.iter().enumerate() {
            let is_cursor = i == self.plugin_cursor;
            let check_icon = if plugin.enabled { " ◉ " } else { " ○ " };
            let cursor_prefix = if is_cursor { " › " } else { "   " };

            let mut lines = Vec::new();
            if is_cursor {
                let check_style = Style::default()
                    .fg(if plugin.enabled {
                        Color::Black
                    } else {
                        Color::DarkGray
                    })
                    .bg(SELECTION_BACKGROUND)
                    .add_modifier(Modifier::BOLD);

                let desc_part = format!("  — {}", plugin.description);
                let full_row_text =
                    format!("{cursor_prefix}{check_icon}{}{desc_part}", plugin.name);
                let visual_width = UnicodeWidthStr::width(full_row_text.as_str());
                let pad_len = list_inner_width.saturating_sub(visual_width);

                lines.push(Line::from(vec![
                    Span::styled(
                        cursor_prefix,
                        Style::default()
                            .fg(SELECTION_TEXT)
                            .bg(SELECTION_BACKGROUND)
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(check_icon, check_style),
                    Span::styled(
                        plugin.name,
                        Style::default()
                            .fg(SELECTION_TEXT)
                            .bg(SELECTION_BACKGROUND)
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(
                        desc_part,
                        Style::default().fg(SELECTION_TEXT).bg(SELECTION_BACKGROUND),
                    ),
                    Span::styled(
                        " ".repeat(pad_len),
                        Style::default().bg(SELECTION_BACKGROUND),
                    ),
                ]));
                list_items.push(ListItem::new(lines));
            } else {
                let name_style = if plugin.enabled {
                    Style::default().fg(TEXT)
                } else {
                    Style::default().fg(MUTED)
                };
                let check_color = if plugin.enabled { ACCENT } else { MUTED };

                lines.push(Line::from(vec![
                    Span::raw(cursor_prefix),
                    Span::styled(
                        check_icon,
                        Style::default()
                            .fg(check_color)
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(plugin.name, name_style),
                    Span::styled(
                        format!("  — {}", plugin.description),
                        Style::default().fg(MUTED),
                    ),
                ]));
                list_items.push(ListItem::new(lines));
            }
        }

        let allowed_count = self.plugins.iter().filter(|p| p.enabled).count();
        let total_count = self.plugins.len();

        let list_block = Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(TEXT))
            .title(Line::from(vec![
                Span::styled(
                    " Allowed Native Plugins ",
                    Style::default().fg(TEXT).add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    " [ SELECTING ] ",
                    Style::default()
                        .fg(SELECTION_TEXT)
                        .bg(SELECTION_BACKGROUND)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    format!(" [Allowed: {allowed_count} / {total_count} plugins] "),
                    Style::default().fg(TEXT),
                ),
                Span::styled(
                    " [Space: Toggle | a: All | i: Invert] ",
                    Style::default().fg(MUTED),
                ),
            ]));

        let mut list_state = ListState::default();
        list_state.select(Some(self.plugin_cursor));
        frame.render_stateful_widget(
            List::new(list_items).block(list_block),
            chunks[1],
            &mut list_state,
        );

        let action_hint = Paragraph::new(Line::from(vec![
            Span::styled(" [ ⬅ Back (Esc) ] ", Style::default().fg(MUTED)),
            Span::raw("   "),
            Span::styled(
                " [ Enter: Next Step (Launch Target) ➔ ] ",
                Style::default()
                    .fg(SELECTION_TEXT)
                    .bg(SELECTION_BACKGROUND)
                    .add_modifier(Modifier::BOLD),
            ),
        ]))
        .alignment(Alignment::Right);
        frame.render_widget(action_hint, chunks[2]);
    }

    fn render_step_3_launch(&self, frame: &mut ratatui::Frame, area: Rect) {
        let block = Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(SUBTLE))
            .title(Span::styled(
                " Step 3: Choose Environment to Launch ",
                Style::default().fg(TEXT).add_modifier(Modifier::BOLD),
            ));

        let inner = block.inner(area);
        frame.render_widget(block, area);

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .margin(1)
            .constraints([
                Constraint::Length(2), // Subtitle
                Constraint::Min(8),    // Target list
                Constraint::Length(2), // Action hint
            ])
            .split(inner);

        let subtitle = Paragraph::new(
            "Configurations have been saved successfully.\nSelect an environment to launch Mint AI Agent, or choose Save & Exit."
        ).style(Style::default().fg(MUTED));
        frame.render_widget(subtitle, chunks[0]);

        let list_inner_width = chunks[1].width.saturating_sub(2) as usize;
        let mut list_items = Vec::new();

        for (i, opt) in self.launch_options.iter().enumerate() {
            let is_cursor = i == self.launch_cursor;
            let cursor_prefix = if is_cursor { " › " } else { "   " };
            let radio = if is_cursor { " ◉ " } else { " ○ " };

            let desc_part = format!("  — {}", opt.description);
            let full_row_text = format!("{cursor_prefix}{radio}{}{desc_part}", opt.title);
            let visual_width = UnicodeWidthStr::width(full_row_text.as_str());
            let pad_len = list_inner_width.saturating_sub(visual_width);

            if is_cursor {
                let line = Line::from(vec![
                    Span::styled(
                        cursor_prefix,
                        Style::default()
                            .fg(SELECTION_TEXT)
                            .bg(SELECTION_BACKGROUND)
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(
                        radio,
                        Style::default()
                            .fg(SELECTION_TEXT)
                            .bg(SELECTION_BACKGROUND)
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(
                        opt.title,
                        Style::default()
                            .fg(SELECTION_TEXT)
                            .bg(SELECTION_BACKGROUND)
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(
                        desc_part,
                        Style::default().fg(SELECTION_TEXT).bg(SELECTION_BACKGROUND),
                    ),
                    Span::styled(
                        " ".repeat(pad_len),
                        Style::default().bg(SELECTION_BACKGROUND),
                    ),
                ]);
                list_items.push(ListItem::new(line));
            } else {
                let line = Line::from(vec![
                    Span::raw(cursor_prefix),
                    Span::styled(radio, Style::default().fg(MUTED)),
                    Span::styled(opt.title, Style::default().fg(TEXT)),
                    Span::styled(desc_part, Style::default().fg(MUTED)),
                ]);
                list_items.push(ListItem::new(line));
            }
        }

        let list_block = Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(TEXT))
            .title(Line::from(vec![
                Span::styled(
                    " Launch Targets ",
                    Style::default().fg(TEXT).add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    " [ SELECTING ] ",
                    Style::default()
                        .fg(SELECTION_TEXT)
                        .bg(SELECTION_BACKGROUND)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    " (↑/↓: Choose target | Enter: Execute) ",
                    Style::default().fg(MUTED),
                ),
            ]));

        let mut list_state = ListState::default();
        list_state.select(Some(self.launch_cursor));
        frame.render_stateful_widget(
            List::new(list_items).block(list_block),
            chunks[1],
            &mut list_state,
        );

        let action_hint = Paragraph::new(Line::from(vec![
            Span::styled(" [ ⬅ Back (Esc) ] ", Style::default().fg(MUTED)),
            Span::raw("   "),
            Span::styled(
                " [ Enter: Confirm & Execute ➔ ] ",
                Style::default()
                    .fg(SELECTION_TEXT)
                    .bg(SELECTION_BACKGROUND)
                    .add_modifier(Modifier::BOLD),
            ),
        ]))
        .alignment(Alignment::Right);
        frame.render_widget(action_hint, chunks[2]);
    }

    fn render_footer(&self, frame: &mut ratatui::Frame, area: Rect) {
        let shortcuts = match self.step {
            SetupStep::Tools => {
                "Focus: [Agent Tools] · [↑/↓] Navigate · [Space] Toggle · [a] All · [i] Invert · [Enter] Next · [Esc] Cancel"
            }
            SetupStep::Plugins => {
                "Focus: [Native Plugins] · [↑/↓] Navigate · [Space] Toggle · [a] All · [i] Invert · [Enter] Next · [Esc] Back"
            }
            SetupStep::Launch => {
                "Focus: [Launch Target] · [↑/↓] Select Target · [Enter] Confirm & Execute · [Esc] Back"
            }
        };

        let chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Min(20), Constraint::Length(16)])
            .split(area);

        frame.render_widget(
            Paragraph::new(format!("  {shortcuts}")).style(Style::default().fg(MUTED)),
            chunks[0],
        );
        frame.render_widget(
            Paragraph::new("Mint v1.18.0  ")
                .style(Style::default().fg(SUBTLE))
                .alignment(Alignment::Right),
            chunks[1],
        );
    }

    // ─────────────────────────────────────────────────────────────────────────
    // Key Event Handlers
    // ─────────────────────────────────────────────────────────────────────────

    pub fn handle_key(&mut self, key: crossterm::event::KeyEvent) -> Option<Option<String>> {
        let is_ctrl_c =
            matches!(key.code, KeyCode::Char('c')) && key.modifiers.contains(KeyModifiers::CONTROL);
        if is_ctrl_c {
            return Some(None);
        }

        match self.step {
            SetupStep::Tools => match key.code {
                KeyCode::Up => {
                    if self.tool_cursor > 0 {
                        self.tool_cursor -= 1;
                    } else if !self.tools.is_empty() {
                        self.tool_cursor = self.tools.len() - 1;
                    }
                }
                KeyCode::Down => {
                    if !self.tools.is_empty() && self.tool_cursor < self.tools.len() - 1 {
                        self.tool_cursor += 1;
                    } else {
                        self.tool_cursor = 0;
                    }
                }
                KeyCode::Char(' ') => {
                    if let Some(tool) = self.tools.get_mut(self.tool_cursor) {
                        tool.enabled = !tool.enabled;
                    }
                }
                KeyCode::Char('a') => {
                    for tool in &mut self.tools {
                        tool.enabled = true;
                    }
                }
                KeyCode::Char('i') => {
                    for tool in &mut self.tools {
                        tool.enabled = !tool.enabled;
                    }
                }
                KeyCode::Enter => {
                    self.step = SetupStep::Plugins;
                }
                KeyCode::Esc => {
                    return Some(None);
                }
                _ => {}
            },
            SetupStep::Plugins => match key.code {
                KeyCode::Up => {
                    if self.plugin_cursor > 0 {
                        self.plugin_cursor -= 1;
                    } else if !self.plugins.is_empty() {
                        self.plugin_cursor = self.plugins.len() - 1;
                    }
                }
                KeyCode::Down => {
                    if !self.plugins.is_empty() && self.plugin_cursor < self.plugins.len() - 1 {
                        self.plugin_cursor += 1;
                    } else {
                        self.plugin_cursor = 0;
                    }
                }
                KeyCode::Char(' ') => {
                    if let Some(plugin) = self.plugins.get_mut(self.plugin_cursor) {
                        plugin.enabled = !plugin.enabled;
                    }
                }
                KeyCode::Char('a') => {
                    for plugin in &mut self.plugins {
                        plugin.enabled = true;
                    }
                }
                KeyCode::Char('i') => {
                    for plugin in &mut self.plugins {
                        plugin.enabled = !plugin.enabled;
                    }
                }
                KeyCode::Enter => {
                    let _ = self.save_changes();
                    self.step = SetupStep::Launch;
                }
                KeyCode::Esc => {
                    self.step = SetupStep::Tools;
                }
                _ => {}
            },
            SetupStep::Launch => match key.code {
                KeyCode::Up => {
                    if self.launch_cursor > 0 {
                        self.launch_cursor -= 1;
                    } else if !self.launch_options.is_empty() {
                        self.launch_cursor = self.launch_options.len() - 1;
                    }
                }
                KeyCode::Down => {
                    if !self.launch_options.is_empty()
                        && self.launch_cursor < self.launch_options.len() - 1
                    {
                        self.launch_cursor += 1;
                    } else {
                        self.launch_cursor = 0;
                    }
                }
                KeyCode::Enter => {
                    let _ = self.save_changes();
                    let target_key = self.launch_options[self.launch_cursor]
                        .key
                        .map(|s| s.to_string());
                    return Some(target_key);
                }
                KeyCode::Esc => {
                    self.step = SetupStep::Plugins;
                }
                _ => {}
            },
        }
        None
    }

    pub fn run_with_terminal<B: Backend>(
        &mut self,
        terminal: &mut Terminal<B>,
    ) -> Result<Option<String>> {
        loop {
            terminal.draw(|f| self.render(f))?;

            if event::poll(Duration::from_millis(50))?
                && let Ok(Event::Key(key)) = event::read()
                && key.kind == crossterm::event::KeyEventKind::Press
                && let Some(outcome) = self.handle_key(key)
            {
                return Ok(outcome);
            }
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Public Entrypoint for `mint setup`
// ─────────────────────────────────────────────────────────────────────────────

pub async fn run() -> Result<Option<String>> {
    let config = load_config()?;
    enable_raw_mode()?;
    let mut stdout = std::io::stdout();
    let _ = stdout.execute(EnterAlternateScreen);
    let _ = stdout.execute(Hide);
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut wizard = SetupWizard::new(config);
    let outcome = wizard.run_with_terminal(&mut terminal);

    let _ = disable_raw_mode();
    let mut stdout = std::io::stdout();
    let _ = stdout.execute(Show);
    let _ = stdout.execute(LeaveAlternateScreen);

    match outcome {
        Ok(Some(ref target)) => {
            println!("\x1b[32m✔ Setup complete. Launching {target}...\x1b[0m\n");
        }
        Ok(None) => {
            println!("\x1b[32m✔ Configuration saved. Exiting setup.\x1b[0m\n");
        }
        Err(ref e) => {
            eprintln!("\x1b[31mSetup error: {e}\x1b[0m");
        }
    }

    outcome
}

// ─────────────────────────────────────────────────────────────────────────────
// Unit Tests
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_setup_wizard_initialization() {
        let mut config = MintConfig::default();
        config.disabled_tools = vec!["web_search".to_string(), "video_trim".to_string()];

        let wizard = SetupWizard::new(config);
        assert_eq!(wizard.step, SetupStep::Tools);
        assert_eq!(wizard.tools.len(), 49);

        // Verify disabled tools match config
        let web_search = wizard.tools.iter().find(|t| t.key == "web_search").unwrap();
        assert!(!web_search.enabled);

        let list_files = wizard.tools.iter().find(|t| t.key == "list_files").unwrap();
        assert!(list_files.enabled);

        assert_eq!(wizard.launch_options.len(), 4);
    }

    #[test]
    fn test_tool_toggle_and_shortcuts() {
        let config = MintConfig::default();
        let mut wizard = SetupWizard::new(config);

        let initial_status = wizard.tools[0].enabled;
        let space_key = crossterm::event::KeyEvent::new(KeyCode::Char(' '), KeyModifiers::empty());
        wizard.handle_key(space_key);
        assert_eq!(wizard.tools[0].enabled, !initial_status);

        // Test 'a' (enable all)
        let a_key = crossterm::event::KeyEvent::new(KeyCode::Char('a'), KeyModifiers::empty());
        wizard.handle_key(a_key);
        assert!(wizard.tools.iter().all(|t| t.enabled));

        // Test 'i' (invert)
        let i_key = crossterm::event::KeyEvent::new(KeyCode::Char('i'), KeyModifiers::empty());
        wizard.handle_key(i_key);
        assert!(wizard.tools.iter().all(|t| !t.enabled));
    }

    #[test]
    fn test_setup_step_transitions() {
        let config = MintConfig::default();
        let mut wizard = SetupWizard::new(config);
        assert_eq!(wizard.step, SetupStep::Tools);

        let enter_key = crossterm::event::KeyEvent::new(KeyCode::Enter, KeyModifiers::empty());
        let esc_key = crossterm::event::KeyEvent::new(KeyCode::Esc, KeyModifiers::empty());

        // Tools -> Plugins
        wizard.handle_key(enter_key);
        assert_eq!(wizard.step, SetupStep::Plugins);

        // Plugins -> Launch
        wizard.handle_key(enter_key);
        assert_eq!(wizard.step, SetupStep::Launch);

        // Launch -> Plugins (Esc)
        wizard.handle_key(esc_key);
        assert_eq!(wizard.step, SetupStep::Plugins);

        // Plugins -> Tools (Esc)
        wizard.handle_key(esc_key);
        assert_eq!(wizard.step, SetupStep::Tools);
    }
}

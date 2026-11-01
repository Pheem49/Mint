//! Unit tests for the Full-Screen ChatTui module.
use super::*;

#[cfg(test)]
mod dialog_tests {
    use super::*;
    use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyEventState, KeyModifiers};
    use ratatui::backend::TestBackend;
    use ratatui::layout::Rect;

    fn saved_activity_fixture(
        activity: serde_json::Value,
        status: &str,
    ) -> (std::path::PathBuf, mint_core::MemoryStore) {
        let root = std::env::temp_dir().join(format!(
            "mint-resume-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&root).unwrap();
        let memory = mint_core::MemoryStore::open(root.join("memory.sqlite"));
        let turn = memory.start_turn("resume-details", "Check files").unwrap();
        memory.claim_turn("resume-details", turn).unwrap();
        memory
            .set_interaction_agent_activity_json(turn, &activity.to_string())
            .unwrap();
        if status == "completed" {
            memory
                .finish_turn(turn, "Saved answer", "test", "model", None)
                .unwrap();
        } else {
            memory.end_turn(turn, status).unwrap();
        }
        (root, memory)
    }

    #[test]
    fn resume_restores_tool_results_thoughts_and_run_summary_without_duplicates() {
        let command = "python3 - <<'EOF'\nprint('complete command')\nEOF";
        let activity = serde_json::json!([
            {"type":"ExtendedThinking","data":{"id":"thought-one","thought":"Saved reasoning","elapsed_ms":1900}},
            {"type":"ToolStart","data":{"callId":"one","action":"run_shell","input":{"command":command}}},
            {"type":"ToolEnd","data":{"callId":"one","action":"run_shell","input":{"command":command},"result":"exit: 2\nstderr:\nfull saved error", "elapsed_ms":3400}},
            {"type":"RunCompleted","data":{"summary":{"runId":"r1","task":"Check files","outcome":"FAILED","totalTokens":100,"toolCallsCount":1,"filesChanged":[],"durationSecs":16.2,"toolTimeline":[{"step":1,"action":"run_shell","target":"python3","success":false,"retried":false}],"retriesCount":0}}}
        ]);
        let (root, memory) = saved_activity_fixture(activity, "completed");
        let mut state = ChatViewState::default();
        for _ in 0..2 {
            state.reload_transcript_with(&memory, "resume-details", &root);
            assert_eq!(state.tools.len(), 1);
            assert_eq!(state.thoughts.len(), 1);
            let text = state
                .transcript
                .iter()
                .map(|entry| entry.text.as_str())
                .collect::<Vec<_>>()
                .join("\n");
            assert!(text.contains("run_shell failed · 3.4s · exit 2"));
            assert_eq!(text.matches("Thought · 1.9s").count(), 1);
            assert!(text.contains("✗ Failed"));
            assert!(text.contains("16.2s"));
            assert!(!state.tools[0].running());
            assert!(state.handle_thought_key(ctrl_t()));
            assert_eq!(
                state.thought_modal.as_ref().unwrap().lines[1..],
                vec!["Saved reasoning"]
            );
            state.handle_thought_key(make_key(KeyCode::Esc));
            let mut open = make_key(KeyCode::Char('o'));
            open.modifiers = KeyModifiers::CONTROL;
            assert!(state.handle_tool_key(open));
            let body = &state.dialog.as_ref().unwrap().body;
            assert!(body.contains(command));
            assert!(body.contains("full saved error"));
            state.handle_tool_key(make_key(KeyCode::Esc));
        }
        assert!(!state.refresh_shared_transcript_with(&memory));
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn resume_switching_sessions_closes_previous_tool_details() {
        let (root, memory) = saved_activity_fixture(
            serde_json::json!([
                {"type":"ToolEnd","data":{"callId":"old","action":"run_shell","input":{"command":"echo previous-session"},"result":"exit: 0"}}
            ]),
            "completed",
        );
        let mut state = ChatViewState::default();
        state.reload_transcript_with(&memory, "resume-details", &root);
        let mut key = make_key(KeyCode::Char('o'));
        key.modifiers = KeyModifiers::CONTROL;
        assert!(state.handle_tool_key(key));
        assert!(state.dialog.is_some());
        state.reload_transcript_with(&memory, "different-session", &root);
        assert!(
            state.dialog.is_none(),
            "previous session tool details remained open"
        );
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn resume_legacy_activity_does_not_invent_zero_second_tool_duration() {
        let (root, memory) = saved_activity_fixture(
            serde_json::json!([
                {"type":"ToolStart","data":{"action":"run_shell","input":{"command":"echo legacy"}}},
                {"type":"ToolEnd","data":{"action":"run_shell","input":{"command":"echo legacy"},"result":"exit: 0\nlegacy output"}}
            ]),
            "completed",
        );
        let mut state = ChatViewState::default();
        state.reload_transcript_with(&memory, "resume-details", &root);
        assert_eq!(state.tools.len(), 1);
        assert!(state.tools[0].heading().contains("time unavailable"));
        assert!(!state.tools[0].running());
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn resume_restores_web_search_sources_in_transcript() {
        let (root, memory) = saved_activity_fixture(
            serde_json::json!([
                {"type":"ToolStart","data":{"callId":"call_1","action":"web_search","input":{"query":"rust ratatui"}}},
                {"type":"ToolEnd","data":{"callId":"call_1","action":"web_search","input":{"query":"rust ratatui"},"result":"1. Ratatui Docs\n   URL: https://ratatui.rs\n   Terminal UI library"}}
            ]),
            "completed",
        );
        let mut state = ChatViewState::default();
        state.reload_transcript_with(&memory, "resume-details", &root);
        let sources_entry = state.transcript.iter().find(|entry| {
            entry.role == TranscriptRole::Notice && entry.text.starts_with("Sources:\n")
        });
        assert!(
            sources_entry.is_some(),
            "Sources notice should be reconstructed upon resume"
        );
        let text = &sources_entry.unwrap().text;
        assert!(text.contains("1. Ratatui Docs\n   https://ratatui.rs"));
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn resume_interrupted_activity_keeps_partial_thought_and_never_restarts_spinner() {
        let events: Vec<mint_core::AgentProgress> = serde_json::from_value(serde_json::json!([
            {"type":"ThinkingDelta","data":{"id":"partial","delta":"unfinished ","elapsed_ms":400}},
            {"type":"ThinkingDelta","data":{"id":"partial","delta":"reasoning","elapsed_ms":700}},
            {"type":"ToolStart","data":{"callId":"unfinished","action":"run_shell","input":{"command":"sleep 100"}}}
        ])).unwrap();
        let activity =
            serde_json::to_value(mint_core::orchestration::compact_agent_progress(&events))
                .unwrap();
        let (root, memory) = saved_activity_fixture(activity, "interrupted");
        let mut state = ChatViewState::default();
        state.reload_transcript_with(&memory, "resume-details", &root);
        assert_eq!(state.tools.len(), 1);
        assert!(!state.tools[0].running());
        assert!(state.tools[0].heading().contains("interrupted"));
        assert!(super::super::tool_activity::running_lines(&state.tools, 80, 0).is_empty());
        assert!(state.handle_thought_key(ctrl_t()));
        assert_eq!(
            state.thought_modal.as_ref().unwrap().lines[1..],
            vec!["unfinished reasoning"]
        );
        state.reload_transcript_with(&memory, "another-session", &root);
        assert!(state.thoughts.is_empty());
        assert!(state.tools.is_empty());
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn device_saved_activity_survives_tui_reload_and_external_sync() {
        let root = std::env::temp_dir().join(format!(
            "mint-device-tui-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&root).unwrap();
        let memory = mint_core::MemoryStore::open(root.join("memory.sqlite"));
        let chat = "device-history";
        let mut state = ChatViewState {
            chat_id: chat.into(),
            current_dir: root.clone(),
            ..Default::default()
        };
        state.reload_transcript_with(&memory, chat, &root);
        let turn = memory.start_turn(chat, "Move motor").unwrap();
        memory.claim_turn(chat, turn).unwrap();
        let activity = serde_json::json!([{"type":"DeviceState","data":{"callId":"one","server":"motor","tool":"move","update":{"state":"verified","target":{"rpm":100},"actual":{"rpm":100},"elapsedSecs":2,"message":"Observed motor"}}}]);
        memory
            .set_interaction_agent_activity_json(turn, &activity.to_string())
            .unwrap();
        memory
            .finish_turn(turn, "done", "test", "model", None)
            .unwrap();
        assert!(state.refresh_shared_transcript_with(&memory));
        assert!(
            state
                .transcript
                .iter()
                .any(|entry| entry.text.contains("Verified")),
            "external sync omitted device activity"
        );
        assert!(!state.refresh_shared_transcript_with(&memory));
        state.reload_transcript_with(&memory, chat, &root);
        assert_eq!(
            state
                .transcript
                .iter()
                .filter(|entry| entry.text.contains("Verified"))
                .count(),
            1
        );
        let mut terminal = Terminal::new(TestBackend::new(100, 30)).unwrap();
        terminal
            .draw(|frame| {
                state.render(frame);
            })
            .unwrap();
        let rendered: String = terminal
            .backend()
            .buffer()
            .content
            .iter()
            .map(|cell| cell.symbol())
            .collect();
        assert!(rendered.contains("Verified"));
        assert!(rendered.contains("rpm"));
        assert!(rendered.contains("100"));
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn device_and_discovery_events_render_inside_full_screen_tui() {
        let state = Arc::new(Mutex::new(ChatViewState::default()));
        let tui = TuiHandle {
            state: Arc::clone(&state),
            interrupted: Arc::new(AtomicBool::new(false)),
            queued: Arc::new(Mutex::new(Vec::new())),
        };
        let mut events = vec![
            serde_json::json!({"type":"ToolDiscovery","data":{"server":"motor","tool":"set_speed","message":"Reading device schema"}}),
        ];
        for phase in ["accepted", "verified"] {
            events.push(serde_json::json!({"type":"DeviceState","data":{"callId":"one","server":"motor","tool":"set_speed","update":{"state":phase,"target":{"rpm":100},"actual":{"rpm":100},"elapsedSecs":2,"message":"Observed motor"}}}));
        }
        events.push(serde_json::json!({"type":"ToolArtifacts","data":{"callId":"one","artifacts":[],"warnings":["Sample attachment unavailable"]}}));
        for event in events {
            crate::agent::publish_mcp_progress(Some(&tui), &serde_json::from_value(event).unwrap());
        }
        let mut terminal = Terminal::new(TestBackend::new(100, 30)).unwrap();
        terminal
            .draw(|frame| {
                state.lock().unwrap().render(frame);
            })
            .unwrap();
        let rendered: String = terminal
            .backend()
            .buffer()
            .content
            .iter()
            .map(|cell| cell.symbol())
            .collect();
        assert!(rendered.contains("Reading device schema"));
        assert!(rendered.contains("Accepted"));
        assert!(rendered.contains("Verified"));
        assert!(rendered.contains("target"));
        assert!(rendered.contains("actual"));
        assert!(rendered.contains("Sample attachment unavailable"));
    }

    #[test]
    fn tui_suppresses_redundant_execution_update_notices() {
        use mint_core::{ExecutionEvidence, ExecutionState};
        let state = Arc::new(Mutex::new(ChatViewState::default()));
        let tui = TuiHandle {
            state: Arc::clone(&state),
            interrupted: Arc::new(AtomicBool::new(false)),
            queued: Arc::new(Mutex::new(Vec::new())),
        };
        let update = mint_core::AgentProgress::ExecutionUpdate {
            call_id: "test".into(),
            executions: vec![ExecutionEvidence {
                call_id: "test1".into(),
                command: "ls -la".into(),
                state: ExecutionState::Succeeded,
                exit_code: Some(0),
                job_id: None,
                revision: 1,
                check: false,
                condition_verified: None,
            }],
        };
        let published = crate::agent::publish_mcp_progress(Some(&tui), &update);
        assert!(!published, "ExecutionUpdate should be suppressed in TUI");
        let transcript = state.lock().unwrap().transcript.clone();
        assert!(
            !transcript
                .iter()
                .any(|entry| entry.text.contains("[Command]")),
            "TUI transcript must not receive redundant [Command] notices"
        );
    }

    #[test]
    fn mint_watermark_frames_fit_available_terminal_height() {
        for height in [3, 7, WATERMARK_HEIGHT as usize] {
            for frame in 0..WATERMARK_FRAME_COUNT {
                let text = mint_m_watermark(Some(frame), height);
                assert_eq!(text.lines.len(), height);
                assert!(text.lines.iter().all(|line| {
                    line.spans
                        .iter()
                        .map(|span| UnicodeWidthStr::width(span.content.as_ref()))
                        .sum::<usize>()
                        == WATERMARK_WIDTH as usize
                }));
            }
        }
        assert_ne!(
            mint_m_watermark(Some(0), WATERMARK_HEIGHT as usize),
            mint_m_watermark(Some(6), WATERMARK_HEIGHT as usize)
        );
        let front = mint_m_watermark(None, WATERMARK_HEIGHT as usize);
        assert_eq!(front.lines[8].spans[19].content.as_ref(), " ");
        let back = mint_m_watermark(Some(WATERMARK_FRAME_COUNT / 2), WATERMARK_HEIGHT as usize);
        assert_eq!(back.lines[8].spans[19].content.as_ref(), "⣿");
    }

    #[test]
    fn watermark_animation_eases_to_leaf_holds_then_continues_forward_to_m() {
        let at = |ms| watermark_animation_at(std::time::Duration::from_millis(ms));
        assert_eq!(at(0).0, Some(0));
        // Slower movement near the endpoints, faster through the middle.
        assert_eq!(at(300).0, Some(2));
        assert_eq!(at(1500).0, Some(50));
        assert_eq!(at(2700).0, Some(98));
        assert_eq!(
            at(3000),
            (Some(100), Some(std::time::Duration::from_millis(4000)))
        );
        assert_eq!(
            at(3999),
            (Some(100), Some(std::time::Duration::from_millis(4000)))
        );
        assert_eq!(at(4000).0, Some(100));
        assert_eq!(at(4300).0, Some(102));
        assert_eq!(at(5500).0, Some(150));
        assert_eq!(at(6700).0, Some(198));
        assert_eq!(
            at(6999),
            (Some(0), Some(std::time::Duration::from_millis(7000)))
        );
        assert_eq!(at(7000), (None, None));
    }

    #[test]
    fn short_sent_user_message_starts_after_label_on_the_same_row() {
        let mut state = ChatViewState::default();
        state
            .transcript
            .push(TranscriptEntry::new(TranscriptRole::User, "ok เข้าใจละ"));
        let text = state.transcript_text(58);
        let mut terminal = Terminal::new(TestBackend::new(58, 4)).unwrap();
        terminal
            .draw(|frame| {
                frame.render_widget(
                    Paragraph::new(text.clone()).wrap(Wrap { trim: false }),
                    frame.area(),
                );
            })
            .unwrap();
        let buffer = terminal.backend().buffer();
        let first: String = (0..58).map(|x| buffer[(x, 0)].symbol()).collect();
        assert!(first.contains("You › ok เข้าใจละ"));
        assert_eq!(
            buffer[(1, 1)].bg,
            Color::Reset,
            "short messages should occupy one shaded row"
        );
        assert_eq!(state.plain_transcript_lines(), vec!["You › ok เข้าใจละ", ""]);
    }

    #[test]
    fn sent_user_message_background_covers_wrapped_text_at_terminal_width() {
        let mut state = ChatViewState::default();
        state.transcript.push(TranscriptEntry::new(
            TranscriptRole::User,
            format!("{}\nข้อความของฉัน", "hello ".repeat(24)),
        ));
        state
            .transcript
            .push(TranscriptEntry::new(TranscriptRole::Assistant, "Reply"));
        for width in [60, 100] {
            let mut terminal = Terminal::new(TestBackend::new(width, 40)).unwrap();
            terminal
                .draw(|frame| {
                    state.render(frame);
                })
                .unwrap();
            let buffer = terminal.backend().buffer();
            let rows: Vec<String> = (0..40)
                .map(|y| (0..width).map(|x| buffer[(x, y)].symbol()).collect())
                .collect();
            let start = rows.iter().position(|row| row.contains("You ›")).unwrap();
            let end = rows.iter().position(|row| row.contains("Mint ›")).unwrap();
            let padding = if width >= 80 { 2 } else { 1 };
            for y in start..end - 1 {
                for x in padding..width - padding {
                    assert_eq!(
                        buffer[(x, y as u16)].bg,
                        Color::Rgb(64, 64, 64),
                        "sent message background must fill every visual row, including padding"
                    );
                }
                assert!(!rows[y].contains(['╭', '╮', '╰', '╯', '│']));
            }
            assert_eq!(
                buffer[(padding, end as u16)].bg,
                Color::Reset,
                "assistant messages must remain outside the user background"
            );
            let mut light_buffer = buffer.clone();
            crate::terminal_theme::apply_tui_theme(
                &mut light_buffer,
                crate::terminal_theme::TuiTheme::Light,
            );
            assert_eq!(
                light_buffer[(padding, start as u16)].bg,
                Color::Rgb(225, 225, 225)
            );
            let mut auto_buffer = buffer.clone();
            crate::terminal_theme::apply_tui_theme(
                &mut auto_buffer,
                crate::terminal_theme::TuiTheme::Auto,
            );
            assert_eq!(
                auto_buffer[(padding + 1, (start + 1) as u16)].fg,
                Color::White,
                "text on the dark user background must keep its contrast in Auto theme"
            );
            assert!(
                end - start > 3,
                "long user text must wrap within the background"
            );
            assert_eq!(
                rows[start..end - 1]
                    .iter()
                    .map(|row| row.matches("hello").count())
                    .sum::<usize>(),
                24
            );
            assert!(
                rows[start..end - 1]
                    .iter()
                    .any(|row| row.contains("ข้อความของฉัน"))
            );
        }
        // Decorative boundaries must not enter keyboard copy or shift selection indices.
        state.selection_anchor = 0;
        state.selection_head = 1;
        assert_eq!(
            state.selected_text(),
            format!("You › {}\nข้อความของฉัน", "hello ".repeat(24))
        );
    }

    fn make_key(code: KeyCode) -> KeyEvent {
        KeyEvent {
            code,
            modifiers: KeyModifiers::empty(),
            kind: KeyEventKind::Press,
            state: KeyEventState::empty(),
        }
    }

    #[test]
    fn slash_suggestion_finds_description_and_completes_executable_token() {
        let mut state = ChatViewState::default();
        state.set_draft("/statistics".to_owned());
        assert_eq!(
            state.suggestions().first().map(|item| item.0.as_str()),
            Some("/stats")
        );
        assert!(state.apply_selected_suggestion());
        assert_eq!(state.composer_text(), "/stats ");
        state.set_draft("/reasoning".to_owned());
        assert!(
            state
                .suggestions()
                .iter()
                .all(|(token, _)| token.starts_with('/'))
        );
        state.set_draft("/remember a durable fact".to_owned());
        assert!(state.suggestions().is_empty());
        state.set_draft("/st".to_owned());
        assert_eq!(
            state.suggestions().first().map(|item| item.0.as_str()),
            Some("/stats")
        );
    }

    #[test]
    fn skill_suggestion_completes_name_before_task() {
        let workspace = std::env::temp_dir().join(format!(
            "mint-tui-skill-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let skill_dir = workspace.join("skills").join("tui-check-skill");
        std::fs::create_dir_all(&skill_dir).unwrap();
        std::fs::write(
            skill_dir.join("SKILL.md"),
            "---\ndescription: Test skill\n---\n",
        )
        .unwrap();
        let mut state = ChatViewState {
            current_dir: workspace.clone(),
            ..ChatViewState::default()
        };
        state.set_draft("$tui-check".to_owned());
        assert!(
            state
                .suggestions()
                .iter()
                .any(|(name, _)| name == "$tui-check-skill")
        );
        assert!(state.apply_selected_suggestion());
        assert_eq!(
            state.composer.iter().collect::<String>(),
            "$tui-check-skill "
        );
        std::fs::remove_dir_all(workspace).unwrap();
    }

    #[test]
    fn mention_suggestion_replaces_only_the_word_at_cursor() {
        let mut state = ChatViewState {
            current_dir: std::env::current_dir().unwrap(),
            ..ChatViewState::default()
        };
        state.set_draft("Please check @work today".to_owned());
        state.cursor = "Please check @work".chars().count();
        assert!(
            state
                .suggestions()
                .iter()
                .any(|(name, _)| name == "@workspace")
        );
        assert!(state.apply_selected_suggestion());
        assert_eq!(
            state.composer.iter().collect::<String>(),
            "Please check @workspace today"
        );
        assert_eq!(state.cursor, "Please check @workspace ".chars().count());
    }

    #[test]
    fn three_choice_dialog_shows_every_option_with_three_body_lines() {
        let (reply, _) = mpsc::channel();
        let mut state = ChatViewState::default();
        state.dialog = Some(DialogState::new_choice(
            "Local shell command",
            "Command: uname -r && uname -a\nMode: mutating\nBackground: no",
            vec![
                "Yes".to_owned(),
                "Yes, allow for this session".to_owned(),
                "No".to_owned(),
            ],
            None,
            reply,
        ));
        let mut terminal = Terminal::new(TestBackend::new(80, 24)).unwrap();
        terminal
            .draw(|frame| {
                state.render(frame);
            })
            .unwrap();
        let rendered: String = terminal
            .backend()
            .buffer()
            .content
            .iter()
            .map(|cell| cell.symbol())
            .collect();
        assert!(rendered.contains("3. No"));
        assert!(!rendered.contains("more below"));
    }

    #[test]
    fn choice_dialog_uses_available_height_for_more_than_eight_options() {
        let (reply, _) = mpsc::channel();
        let mut state = ChatViewState::default();
        state.dialog = Some(DialogState::new_choice(
            "Choose one",
            "",
            (1..=9).map(|index| format!("Option {index}")).collect(),
            None,
            reply,
        ));
        let mut terminal = Terminal::new(TestBackend::new(80, 32)).unwrap();
        terminal
            .draw(|frame| {
                state.render(frame);
            })
            .unwrap();
        let rendered: String = terminal
            .backend()
            .buffer()
            .content
            .iter()
            .map(|cell| cell.symbol())
            .collect();
        assert!(rendered.contains("Option 9"));
        assert!(!rendered.contains("more below"));
    }

    #[test]
    fn long_skill_diff_keeps_decisions_visible_and_can_be_reviewed_to_the_end() {
        let (reply, _) = mpsc::channel();
        let mut state = ChatViewState::default();
        state.dialog = Some(DialogState::new_choice(
            "Save skill?",
            format!(
                "Changes instructions used in future tasks.\nPath: /workspace/.agents/skills/example/SKILL.md\n\n{}",
                (1..=100)
                    .map(|i| format!("+ Skill instruction {i:03}: {}", "รายละเอียด ".repeat(8)))
                    .collect::<Vec<_>>()
                    .join("\n")
            ),
            vec!["Approve".into(), "Deny".into()],
            None,
            reply,
        ));
        for (width, height) in [(80, 24), (160, 42), (60, 16)] {
            state.dialog.as_mut().unwrap().selected = 0;
            let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
            terminal
                .draw(|frame| {
                    state.render(frame);
                })
                .unwrap();
            let visible = |buffer: &Buffer| {
                buffer
                    .content
                    .iter()
                    .map(|cell| cell.symbol())
                    .collect::<String>()
            };
            let rendered = visible(terminal.backend().buffer());
            assert!(
                rendered.contains("1. Approve"),
                "{width}x{height}: approve hidden"
            );
            assert!(
                rendered.contains("2. Deny"),
                "{width}x{height}: deny hidden"
            );
            assert!(
                rendered.contains("Enter select"),
                "{width}x{height}: hint hidden"
            );
            assert!(rendered.contains("PgUp/PgDn"));
            assert!(rendered.contains("Changes instructions"));
            let dialog = state.dialog.as_mut().unwrap();
            assert_eq!(
                dialog.handle_key(KeyEvent::new(KeyCode::End, KeyModifiers::NONE)),
                None
            );
            assert_eq!(dialog.selected, 0);
            terminal
                .draw(|frame| {
                    state.render(frame);
                })
                .unwrap();
            let rendered = visible(terminal.backend().buffer());
            assert!(
                rendered.contains("Skill instruction 100"),
                "{width}x{height}: last diff line hidden"
            );
            assert!(rendered.contains("2. Deny"));
            route_mouse_wheel(&mut state, event::MouseEventKind::ScrollDown);
            terminal
                .draw(|frame| {
                    state.render(frame);
                })
                .unwrap();
            assert!(visible(terminal.backend().buffer()).contains("Skill instruction 100"));
            route_mouse_wheel(&mut state, event::MouseEventKind::ScrollUp);
            assert_eq!(
                state.scroll_from_bottom, 0,
                "wheel must review the dialog, not chat"
            );
            let dialog = state.dialog.as_mut().unwrap();
            dialog.handle_key(KeyEvent::new(KeyCode::Home, KeyModifiers::NONE));
            terminal
                .draw(|frame| {
                    state.render(frame);
                })
                .unwrap();
            assert!(visible(terminal.backend().buffer()).contains("Changes instructions"));
            let dialog = state.dialog.as_mut().unwrap();
            dialog.handle_key(KeyEvent::new(KeyCode::PageDown, KeyModifiers::NONE));
            terminal
                .draw(|frame| {
                    state.render(frame);
                })
                .unwrap();
            assert!(!visible(terminal.backend().buffer()).contains("Changes instructions"));
            state
                .dialog
                .as_mut()
                .unwrap()
                .handle_key(KeyEvent::new(KeyCode::PageUp, KeyModifiers::NONE));
            terminal
                .draw(|frame| {
                    state.render(frame);
                })
                .unwrap();
            assert!(visible(terminal.backend().buffer()).contains("Changes instructions"));
            let dialog = state.dialog.as_mut().unwrap();
            dialog.handle_key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE));
            assert_eq!(
                dialog.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)),
                Some(DialogAnswer::Choice(1))
            );
            dialog.handle_key(KeyEvent::new(KeyCode::Home, KeyModifiers::NONE));
        }
    }

    #[test]
    fn long_approval_command_keeps_choices_and_hint_visible_in_narrow_terminal() {
        let (reply, _) = mpsc::channel();
        let mut state = ChatViewState::default();
        state.status = vec!["Running shell command".to_owned(); 6];
        state.dialog = Some(DialogState::new_choice(
            "Local shell command",
            concat!(
                "Command: who -a; echo '--- LOGINCTL ---'; loginctl list-sessions 2>&1; ",
                "echo '--- SEATS/USERS ---'; loginctl list-users 2>&1; ",
                "echo '--- UPTIME ---'; uptime -p; uptime -s; ",
                "echo '--- SESSION ENV ---'; echo TYPE=$XDG_SESSION_TYPE ",
                "DESKTOP=$XDG_CURRENT_DESKTOP SESSION=$DESKTOP_SESSION ",
                "WAYLAND=$WAYLAND_DISPLAY DISPLAY=$DISPLAY; ",
                "echo '--- TTY/USER PROCS (top 25 by count) ---'; ",
                "ps -u $USER -o comm= | sort | uniq -c | sort -rn | head -25\n",
                "Mode: mutating\nBackground: no",
            ),
            vec![
                "Yes".to_owned(),
                "Yes, allow for this session".to_owned(),
                "No".to_owned(),
            ],
            None,
            reply,
        ));
        let mut terminal = Terminal::new(TestBackend::new(94, 30)).unwrap();
        terminal
            .draw(|frame| {
                state.render(frame);
            })
            .unwrap();
        let rendered: String = terminal
            .backend()
            .buffer()
            .content
            .iter()
            .map(|cell| cell.symbol())
            .collect();
        assert!(rendered.contains("3. No"));
        assert!(rendered.contains("Enter select"));
        assert!(!rendered.contains("more below"));
    }

    #[test]
    fn background_count_updates_above_composer_and_uses_the_actual_workspace() {
        let root = std::env::temp_dir().join(format!(
            "mint-tui-bg-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir(&root).unwrap();
        let config = mint_core::MintConfig {
            safety_enabled: false,
            sandbox_mode: "off".into(),
            ..Default::default()
        };
        let first = mint_core::bg_shell::start_background(&root, &config, "sleep 30").unwrap();
        let second = mint_core::bg_shell::start_background(&root, &config, "sleep 30").unwrap();
        struct Cleanup(Vec<String>, std::path::PathBuf);
        impl Drop for Cleanup {
            fn drop(&mut self) {
                for id in &self.0 {
                    let _ = mint_core::bg_shell::kill_job(id);
                }
                let _ = std::fs::remove_dir(&self.1);
            }
        }
        let _cleanup = Cleanup(vec![first.id.clone(), second.id.clone()], root.clone());
        let mut state = ChatViewState::default();
        state.current_dir = root.clone();
        state.workspace = format!("{} · branch: Rust", root.display());
        state.set_notice("A temporary notice");
        for width in [160, 40] {
            let expected = "2 Background Terminals running";
            let mut terminal = Terminal::new(TestBackend::new(width, 24)).unwrap();
            let mut composer = None;
            terminal
                .draw(|frame| composer = state.render(frame).composer)
                .unwrap();
            let area = composer.unwrap();
            let rendered: String = (area.x..area.right())
                .map(|x| {
                    terminal
                        .backend()
                        .buffer()
                        .cell(Position::new(x, area.y))
                        .unwrap()
                        .symbol()
                })
                .collect();
            assert!(
                rendered.contains(expected),
                "missing count above composer: {rendered}"
            );
            assert!(
                rendered.trim_end_matches([' ', '─']).ends_with("running"),
                "count must align right: {rendered}"
            );
            let rendered: String = terminal
                .backend()
                .buffer()
                .content
                .iter()
                .map(|cell| cell.symbol())
                .collect();
            assert!(
                rendered.contains(expected),
                "missing job count at width {width}: {rendered}"
            );
        }
        let wait_for_count = |expected| {
            let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
            while mint_core::bg_shell::running_count_for(&root) != expected {
                assert!(std::time::Instant::now() < deadline);
                std::thread::sleep(std::time::Duration::from_millis(20));
            }
        };
        mint_core::bg_shell::kill_job(&first.id).unwrap();
        wait_for_count(1);
        let mut terminal = Terminal::new(TestBackend::new(80, 24)).unwrap();
        terminal
            .draw(|frame| {
                state.render(frame);
            })
            .unwrap();
        let rendered: String = terminal
            .backend()
            .buffer()
            .content
            .iter()
            .map(|cell| cell.symbol())
            .collect();
        assert!(rendered.contains("1 Background Terminal running"));
        mint_core::bg_shell::kill_job(&second.id).unwrap();
        wait_for_count(0);
        terminal
            .draw(|frame| {
                state.render(frame);
            })
            .unwrap();
        let rendered: String = terminal
            .backend()
            .buffer()
            .content
            .iter()
            .map(|cell| cell.symbol())
            .collect();
        assert!(!rendered.contains("Background Terminal"));
    }

    #[test]
    fn live_shell_status_remains_visible_above_a_long_transcript() {
        let mut state = ChatViewState::default();
        state.transcript.push(TranscriptEntry::new(
            TranscriptRole::Assistant,
            "Earlier output\n".repeat(80),
        ));
        state.status = vec![
            "  ● Running 1 shell command…".to_owned(),
            "    └ ⠋ [run_shell] Running command: sleep 1".to_owned(),
            "  Unpacking…".to_owned(),
        ];
        let mut terminal = Terminal::new(TestBackend::new(94, 30)).unwrap();
        terminal
            .draw(|frame| {
                state.render(frame);
            })
            .unwrap();
        let rendered: String = terminal
            .backend()
            .buffer()
            .content
            .iter()
            .map(|cell| cell.symbol())
            .collect();
        assert!(rendered.contains("Running 1 shell command"));
        assert!(rendered.contains("[run_shell] Running command"));
    }

    fn point(column: u16, row: u16) -> ScreenPoint {
        ScreenPoint { row, column }
    }

    struct FakeClipboard {
        fail: bool,
    }

    impl NativeClipboard for FakeClipboard {
        fn set_text(&mut self, _text: String) -> std::result::Result<(), String> {
            if self.fail {
                Err("unavailable".to_owned())
            } else {
                Ok(())
            }
        }
    }

    #[test]
    fn transcript_selection_copies_across_render_chunks_without_losing_rows() {
        let text = format!("start\n{}end", "middle\n".repeat(64));
        let area = Rect::new(0, 3, 8, 3);
        let mut drag =
            TranscriptSelection::from_text(Text::from(text.clone()), area, 0, point(0, 3));
        drag.update(point(7, 5));
        let mut scroll = 63;
        for _ in 0..70 {
            drag.scroll(&mut scroll);
        }
        assert_eq!(drag.text(), text);
    }

    #[test]
    fn transcript_selection_can_copy_the_visible_tail_beyond_u16_rows() {
        let mut lines = vec![Line::raw("earlier"); 66_000];
        lines.extend([
            Line::raw("tail one"),
            Line::raw("tail two"),
            Line::raw("tail end"),
        ]);
        let area = Rect::new(0, 3, 8, 3);
        let mut drag = TranscriptSelection::from_text(Text::from(lines), area, 66_000, point(0, 3));
        drag.update(point(7, 5));
        assert_eq!(drag.text(), "tail one\ntail two\ntail end");
    }

    #[test]
    fn transcript_selection_keeps_the_back_button_and_scrollbar_visible() {
        let area = Rect::new(0, 3, 8, 3);
        let drag = TranscriptSelection::from_text(
            Text::from(
                (0..9)
                    .map(|row| Line::raw(format!("line{row}")))
                    .collect::<Vec<_>>(),
            ),
            area,
            0,
            point(0, 4),
        );
        let mut screen = Buffer::empty(Rect::new(0, 0, 8, 8));
        screen.set_string(2, 5, "Back", Style::default());
        screen.set_string(7, 3, "▲", Style::default());
        drag.paint(&mut screen, Some(Rect::new(2, 5, 4, 1)));
        assert_eq!(screen.cell(Position::new(7, 3)).unwrap().symbol(), "▲");
        assert_eq!(screen.cell(Position::new(2, 5)).unwrap().symbol(), "B");
    }

    #[test]
    fn transcript_drag_scrolls_both_edges_and_keeps_offscreen_text() {
        let area = Rect::new(0, 3, 8, 3);
        let mut drag = TranscriptSelection::from_text(
            Text::from(
                (0..9)
                    .map(|row| Line::raw(format!("line{row}")))
                    .collect::<Vec<_>>(),
            ),
            area,
            0,
            point(0, 4),
        );
        drag.update(point(4, 5));
        let mut scroll = 6;
        for _ in 0..10 {
            drag.scroll(&mut scroll);
        }
        assert_eq!(scroll, 0);
        assert_eq!(
            drag.text(),
            "line1\nline2\nline3\nline4\nline5\nline6\nline7\nline8"
        );
        let mut screen = Buffer::empty(Rect::new(0, 0, 8, 8));
        screen.set_string(0, 3, "new text", Style::default());
        drag.paint(&mut screen, None);
        assert_eq!(screen.cell(Position::new(0, 3)).unwrap().symbol(), "l");
        assert_eq!(
            screen.cell(Position::new(0, 3)).unwrap().bg,
            crate::terminal_theme::BLUE
        );
        assert_eq!(screen.cell(Position::new(0, 2)).unwrap().symbol(), " ");
        drag.update(point(0, 3));
        for _ in 0..10 {
            drag.scroll(&mut scroll);
        }
        assert_eq!(scroll, 6);
        assert_eq!(drag.text(), "line0\nl");
        drag.dragging = false;
        drag.update(point(4, 5));
        assert!(!drag.scroll(&mut scroll));
        assert_eq!(scroll, 6);
    }

    #[test]
    fn transcript_drag_interior_does_not_scroll() {
        let area = Rect::new(0, 3, 8, 3);
        let mut drag = TranscriptSelection::from_text(
            Text::from(vec![Line::raw(""); 9]),
            area,
            2,
            point(0, 4),
        );
        drag.update(point(4, 4));
        let mut scroll = 4;
        assert!(!drag.scroll(&mut scroll));
        assert_eq!(scroll, 4);
    }

    #[test]
    fn mouse_selection_requires_a_drag_and_normalizes_direction() {
        let mut selection = MouseSelection::default();
        selection.start(point(7, 4));
        assert!(!selection.finish(point(7, 4)));
        assert_eq!(selection.range(), None);

        selection.start(point(7, 4));
        selection.update(point(2, 1));
        assert!(selection.finish(point(2, 1)));
        assert_eq!(selection.range(), Some((point(2, 1), point(7, 4))));
    }

    #[test]
    fn screen_selection_copies_visual_rows_and_trims_only_the_end() {
        let mut buffer = Buffer::empty(Rect::new(0, 0, 8, 2));
        buffer.set_string(0, 0, " hello", Style::default());
        buffer.set_string(0, 1, "world", Style::default());
        let selection = MouseSelection {
            anchor: Some(point(1, 0)),
            head: Some(point(2, 1)),
            dragging: false,
            had_drag: true,
        };

        assert_eq!(selected_screen_text(&buffer, selection), "hello\nwor");
    }

    #[test]
    fn screen_selection_includes_a_wide_grapheme_from_its_continuation_cell() {
        let mut buffer = Buffer::empty(Rect::new(0, 0, 8, 1));
        buffer.set_string(0, 0, "A🙂B ก้", Style::default());
        let selection = MouseSelection {
            anchor: Some(point(2, 0)),
            head: Some(point(2, 0)),
            dragging: false,
            had_drag: true,
        };

        assert_eq!(selected_screen_text(&buffer, selection), "🙂");
    }

    #[test]
    fn selection_overlay_changes_style_without_changing_symbols() {
        let mut buffer = Buffer::empty(Rect::new(0, 0, 5, 1));
        buffer.set_string(
            0,
            0,
            "Mint",
            Style::default().fg(crate::terminal_theme::ACCENT),
        );
        let before = buffer
            .content
            .iter()
            .map(|cell| cell.symbol().to_owned())
            .collect::<Vec<_>>();
        let selection = MouseSelection {
            anchor: Some(point(1, 0)),
            head: Some(point(2, 0)),
            dragging: false,
            had_drag: true,
        };

        apply_mouse_selection(&mut buffer, selection);

        let after = buffer
            .content
            .iter()
            .map(|cell| cell.symbol().to_owned())
            .collect::<Vec<_>>();
        assert_eq!(after, before);
        assert!(!buffer[(0, 0)].modifier.contains(Modifier::REVERSED));
        assert!(buffer[(1, 0)].modifier.contains(Modifier::REVERSED));
        assert!(buffer[(2, 0)].modifier.contains(Modifier::REVERSED));
        assert!(!buffer[(3, 0)].modifier.contains(Modifier::REVERSED));
    }

    #[test]
    fn completed_selection_is_invalidated_when_selected_cells_change() {
        let mut previous = Buffer::empty(Rect::new(0, 0, 5, 1));
        previous.set_string(0, 0, "Mint", Style::default());
        let selection = MouseSelection {
            anchor: Some(point(1, 0)),
            head: Some(point(2, 0)),
            dragging: false,
            had_drag: true,
        };
        let unchanged = previous.clone();
        assert!(selected_cells_match(&previous, &unchanged, selection));

        let mut changed_inside = previous.clone();
        changed_inside.set_string(1, 0, "XX", Style::default());
        assert!(!selected_cells_match(&previous, &changed_inside, selection));

        let mut changed_outside = previous.clone();
        changed_outside.set_string(4, 0, "!", Style::default());
        assert!(selected_cells_match(&previous, &changed_outside, selection));
    }

    #[test]
    fn clipboard_prefers_native_and_falls_back_to_osc52() {
        let mut native = TextClipboard {
            native: Some(Box::new(FakeClipboard { fail: false })),
            initialization_attempted: true,
        };
        let mut native_output = Vec::new();
        assert_eq!(
            native.copy("Mint", &mut native_output).unwrap(),
            CopyMethod::Native
        );
        assert!(native_output.is_empty());

        let mut fallback = TextClipboard {
            native: Some(Box::new(FakeClipboard { fail: true })),
            initialization_attempted: true,
        };
        let mut osc52_output = Vec::new();
        assert_eq!(
            fallback.copy("Mint", &mut osc52_output).unwrap(),
            CopyMethod::Osc52
        );
        assert_eq!(
            String::from_utf8(osc52_output).unwrap(),
            "\u{1b}]52;c;TWludA==\u{7}"
        );
    }

    #[test]
    fn terminal_setup_keeps_wheel_and_history_navigation_distinct() {
        let mut output = Vec::new();
        setup_terminal_output(&mut output).expect("terminal setup should render ANSI commands");

        let output = String::from_utf8(output).expect("terminal commands should be UTF-8");
        assert!(output.contains("\u{1b}[?1049h"));
        assert!(
            output.contains("\u{1b}[?1000h")
                && output.contains("\u{1b}[?1002h")
                && output.contains("\u{1b}[?1003h")
                && output.contains("\u{1b}[?1006h"),
            "mouse reporting is required to distinguish wheel events from Up/Down keys"
        );
        assert!(output.contains("\u{1b}[?2004h"));
    }

    #[test]
    fn mouse_wheel_routes_to_transcript_or_open_thought_panel() {
        let mut state = ChatViewState::default();
        route_mouse_wheel(&mut state, event::MouseEventKind::ScrollUp);
        assert_eq!(state.scroll_from_bottom, 3);

        state.thought_modal = Some(ThoughtModalState {
            elapsed_str: "1.0s".to_owned(),
            lines: vec!["thought".to_owned()],
            scroll: 4,
        });
        route_mouse_wheel(&mut state, event::MouseEventKind::ScrollUp);
        assert_eq!(state.thought_modal.as_ref().unwrap().scroll, 1);
        route_mouse_wheel(&mut state, event::MouseEventKind::ScrollDown);
        assert_eq!(state.thought_modal.as_ref().unwrap().scroll, 4);
    }

    #[test]
    fn back_to_bottom_appears_only_while_scrolled_up_and_click_returns_to_latest() {
        let mut state = ChatViewState::default();
        state.push_user(
            (0..60)
                .map(|n| format!("line {n}"))
                .collect::<Vec<_>>()
                .join("\n"),
        );
        let mut terminal = Terminal::new(TestBackend::new(80, 24)).unwrap();
        let mut button = None;
        terminal
            .draw(|frame| button = state.render(frame).back_to_bottom)
            .unwrap();
        assert!(button.is_none());

        state.scroll_from_bottom = 10;
        terminal
            .draw(|frame| button = state.render(frame).back_to_bottom)
            .unwrap();
        let area = button.expect("button should appear above the composer");
        let rendered: String = terminal
            .backend()
            .buffer()
            .content
            .iter()
            .map(|cell| cell.symbol())
            .collect();
        assert!(rendered.contains("Back to bottom"));
        assert!(!activate_back_to_bottom(
            &mut state,
            button,
            event::MouseEventKind::Down(event::MouseButton::Left),
            ScreenPoint {
                row: area.y,
                column: area.x.saturating_sub(1)
            },
        ));
        assert_eq!(state.scroll_from_bottom, 10);
        assert!(activate_back_to_bottom(
            &mut state,
            button,
            event::MouseEventKind::Down(event::MouseButton::Left),
            ScreenPoint {
                row: area.y,
                column: area.x
            },
        ));
        assert_eq!(state.scroll_from_bottom, 0);
        terminal
            .draw(|frame| button = state.render(frame).back_to_bottom)
            .unwrap();
        assert!(button.is_none());
    }

    fn thought_screen(state: &ChatViewState, width: u16, height: u16) -> Vec<String> {
        let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
        terminal
            .draw(|frame| {
                state.render(frame);
            })
            .unwrap();
        terminal
            .backend()
            .buffer()
            .content
            .chunks(width as usize)
            .map(|cells| cells.iter().map(|cell| cell.symbol()).collect())
            .collect()
    }

    fn ctrl_t() -> KeyEvent {
        let mut key = make_key(KeyCode::Char('t'));
        key.modifiers = KeyModifiers::CONTROL;
        key
    }

    fn tool_event(value: serde_json::Value) -> mint_core::AgentProgress {
        serde_json::from_value(value).unwrap()
    }

    #[test]
    fn running_tool_status_stays_above_composer_with_command_and_spinner() {
        let state = Arc::new(Mutex::new(ChatViewState::default()));
        let tui = ChatTui::handle(state.clone());
        tui.tool_progress(&tool_event(serde_json::json!({"type":"ToolStart","data":{"callId":"a","action":"run_shell","input":{"command":"sleep 5 # live-command-marker"}}})));
        {
            let mut state = state.lock().unwrap();
            for _ in 0..35 {
                state.push_notice("older transcript row");
            }
            state.scroll_from_bottom = 20;
            state.status = (0..12).map(|index| format!("Plan row {index}")).collect();
            let rows = thought_screen(&state, 100, 30);
            let pinned = rows[22..26].join("\n");
            assert!(
                pinned.contains("Running 1 shell command"),
                "missing active summary: {pinned}"
            );
            assert!(
                pinned.contains("live-command-marker"),
                "missing running command: {pinned}"
            );
            assert!(
                pinned.chars().any(|ch| "⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏".contains(ch)),
                "missing animated spinner: {pinned}"
            );
        }
        tui.tool_progress(&tool_event(serde_json::json!({"type":"ToolEnd","data":{"callId":"a","action":"run_shell","input":{"command":"sleep 5 # live-command-marker"},"result":"exit: 0"}})));
        let state = state.lock().unwrap();
        assert!(
            !thought_screen(&state, 100, 30)[22..26]
                .join("\n")
                .contains("live-command-marker")
        );
    }

    #[test]
    fn compact_tools_details_cannot_replace_an_approval_dialog() {
        let state = Arc::new(Mutex::new(ChatViewState::default()));
        let tui = ChatTui::handle(state.clone());
        tui.tool_progress(&tool_event(serde_json::json!({"type":"ToolStart","data":{"callId":"a","action":"run_shell","input":{"command":"echo hi"}}})));
        {
            let mut state = state.lock().unwrap();
            let mut key = make_key(KeyCode::Char('o'));
            key.modifiers = KeyModifiers::CONTROL;
            assert!(state.handle_tool_key(key));
            let (reply, _) = mpsc::channel();
            state.dialog = Some(DialogState::new_choice(
                "Local shell command",
                "Approval required",
                vec!["Yes".into(), "No".into()],
                None,
                reply,
            ));
        }
        tui.tool_progress(&tool_event(serde_json::json!({"type":"ToolEnd","data":{"callId":"a","action":"run_shell","input":{"command":"echo hi"},"result":"exit: 0"}})));
        let state = state.lock().unwrap();
        assert_eq!(state.dialog.as_ref().unwrap().title, "Local shell command");
        assert!(state.tool_viewer.is_none());
    }

    #[test]
    fn identical_legacy_starts_keep_separate_running_cards() {
        let mut state = ChatViewState::default();
        let start = tool_event(
            serde_json::json!({"type":"ToolStart","data":{"action":"read_file","input":{"path":"same"}}}),
        );
        state.record_tool_progress(&start);
        state.record_tool_progress(&start);
        assert_eq!(state.tools.len(), 2);
        let end = tool_event(
            serde_json::json!({"type":"ToolEnd","data":{"action":"read_file","input":{"path":"same"},"result":"contents"}}),
        );
        state.record_tool_progress(&end);
        assert_eq!(state.tools.iter().filter(|tool| tool.running()).count(), 1);
        state.record_tool_progress(&end);
        assert_eq!(state.tools.len(), 2);
        assert!(state.tools.iter().all(|tool| !tool.running()));
    }

    #[test]
    fn compact_tools_count_only_running_calls_and_update_existing_card() {
        let state = Arc::new(Mutex::new(ChatViewState::default()));
        let tui = ChatTui::handle(state.clone());
        for (id, action) in [("a", "run_shell"), ("b", "run_shell"), ("c", "read_file")] {
            tui.tool_progress(&tool_event(serde_json::json!({"type":"ToolStart","data":{"callId":id,"action":action,"input":{"command":"echo hi"}}})));
        }
        assert!(
            thought_screen(&state.lock().unwrap(), 100, 30)
                .join("\n")
                .contains("Running 3 tasks · 2 shell commands · 1 tool")
        );
        tui.tool_progress(&tool_event(serde_json::json!({"type":"ToolEnd","data":{"callId":"a","action":"run_shell","input":{"command":"echo hi"},"result":"exit: 0\nstdout:\nhi"}})));
        let state = state.lock().unwrap();
        assert_eq!(state.transcript.len(), 3);
        let screen = thought_screen(&state, 100, 30).join("\n");
        assert!(screen.contains("run_shell completed"));
        assert!(screen.contains("Running 2 tasks · 1 shell command · 1 tool"));
        assert!(!screen.contains("stdout:"));
    }

    #[test]
    fn compact_tools_hide_multiline_commands_and_keep_full_details_accessible() {
        let state = Arc::new(Mutex::new(ChatViewState::default()));
        let tui = ChatTui::handle(state.clone());
        let command = format!(
            "python3 - <<'EOF'\n{}\nEOF",
            "print('secret-detail')\n".repeat(40)
        );
        tui.tool_progress(&tool_event(serde_json::json!({"type":"ToolStart","data":{"callId":"a","action":"run_shell","input":{"command":command}}})));
        let mut state = state.lock().unwrap();
        assert!(
            !thought_screen(&state, 80, 24)
                .join("\n")
                .contains("secret-detail")
        );
        let mut key = make_key(KeyCode::Char('o'));
        key.modifiers = KeyModifiers::CONTROL;
        assert!(state.handle_tool_key(key));
        assert!(state.dialog.as_ref().unwrap().body.contains(&command));
        assert!(state.handle_tool_key(make_key(KeyCode::Esc)));
        assert!(state.dialog.is_none());
    }

    #[test]
    fn compact_tools_match_parallel_results_by_id_and_keep_failures_red() {
        let state = Arc::new(Mutex::new(ChatViewState::default()));
        let tui = ChatTui::handle(state.clone());
        for id in ["a", "b"] {
            tui.tool_progress(&tool_event(serde_json::json!({"type":"ToolStart","data":{"callId":id,"action":"run_shell","input":{"command":"same-command"}}})));
        }
        tui.tool_progress(&tool_event(serde_json::json!({"type":"ToolEnd","data":{"callId":"b","action":"run_shell","input":{"command":"same-command"},"result":"exit: 2\nstderr:\nfull failure"}})));
        let state = state.lock().unwrap();
        assert_eq!(state.transcript.len(), 2);
        assert_eq!(
            state.transcript[1].rendered_lines[0].spans[0].style.fg,
            Some(Color::Red)
        );
        assert!(
            thought_screen(&state, 80, 24)
                .join("\n")
                .contains("Running 1 shell command")
        );
    }

    #[test]
    fn live_thought_timer_advances_without_deltas_and_preview_fits_small_terminals() {
        let state = Arc::new(Mutex::new(ChatViewState::default()));
        let handle = ChatTui::handle(state.clone());
        handle.append_thought_delta("one", "checking code", std::time::Duration::ZERO);
        let mut state = state.lock().unwrap();
        state.thoughts.last_mut().unwrap().updated_at =
            Some(std::time::Instant::now() - std::time::Duration::from_secs(3));
        assert!(
            thought_screen(&state, 80, 24)
                .join("\n")
                .contains("Thinking · 3.0s")
        );
        for (width, height) in [(40, 16), (60, 20), (80, 24)] {
            let rows = thought_screen(&state, width, height);
            let top = rows
                .iter()
                .position(|line| line.trim_start().starts_with('┌'))
                .unwrap();
            let bottom = rows
                .iter()
                .enumerate()
                .skip(top + 1)
                .find(|(_, line)| line.trim_start().starts_with('└'))
                .unwrap()
                .0;
            assert!(bottom - top < 6);
            assert!(rows.join("\n").contains("checking code"));
        }
        assert!(state.handle_thought_key(ctrl_t()));
        for (width, height) in [(20, 8), (40, 12), (80, 24)] {
            thought_screen(&state, width, height);
        }
    }

    #[test]
    fn live_thought_history_keeps_steps_once_and_remains_accessible_next_turn() {
        let state = Arc::new(Mutex::new(ChatViewState::default()));
        let handle = ChatTui::handle(state.clone());
        handle.append_thought_delta("one", "first partial", std::time::Duration::ZERO);
        handle.finish_thought(
            Some("one"),
            "first complete",
            std::time::Duration::from_secs(1),
        );
        handle.append_thought_delta("two", "second partial", std::time::Duration::ZERO);
        assert!(state.lock().unwrap().handle_thought_key(ctrl_t()));
        handle.finish_thought(
            Some("two"),
            "second complete",
            std::time::Duration::from_secs(2),
        );
        {
            let state = state.lock().unwrap();
            let text = state.thought_modal.as_ref().unwrap().lines.join("\n");
            assert_eq!(text.matches("first complete").count(), 1);
            assert_eq!(text.matches("second complete").count(), 1);
            assert!(!text.contains("partial"));
            let screen = thought_screen(&state, 100, 30).join("\n");
            assert!(screen.contains("Thought Process · 3.0s"), "{screen}");
            assert!(screen.contains("Thought 1 · 1.0s"), "{screen}");
            assert!(screen.contains("Thought 2 · 2.0s"), "{screen}");
            assert_eq!(state.transcript.len(), 2);
        }
        handle.reset_thoughts();
        let mut state = state.lock().unwrap();
        assert!(state.thought_modal.is_none());
        assert!(state.handle_thought_key(ctrl_t()));
        assert!(
            state
                .thought_modal
                .as_ref()
                .unwrap()
                .lines
                .join("\n")
                .contains("first complete")
        );
    }

    #[test]
    fn live_thought_wrapped_tail_preserves_latest_unicode_text() {
        let state = Arc::new(Mutex::new(ChatViewState::default()));
        let handle = ChatTui::handle(state.clone());
        handle.append_thought_delta(
            "one",
            &format!("{}\nlatest ข้อความ 🙂", "older words ".repeat(30)),
            std::time::Duration::ZERO,
        );
        let state = state.lock().unwrap();
        let screen = thought_screen(&state, 40, 24).join("\n");
        assert!(screen.contains("latest ข้อความ 🙂"));
        assert!(!screen.contains("older words older words older words older words"));
    }

    #[test]
    fn live_thought_preview_keeps_only_latest_rows_without_appending_transcript() {
        let state = Arc::new(Mutex::new(ChatViewState::default()));
        let handle = ChatTui::handle(state.clone());
        assert!(
            !thought_screen(&state.lock().unwrap(), 80, 24)
                .join("\n")
                .contains("Thinking ·")
        );
        for index in 1..=20 {
            handle.append_thought_delta(
                "one",
                &format!("reasoning row {index:02}\n"),
                std::time::Duration::from_millis(1200),
            );
        }
        let state = state.lock().unwrap();
        let rows = thought_screen(&state, 80, 24);
        let title = rows
            .iter()
            .position(|line| line.contains("Thinking · 1.2s"))
            .expect("live thinking header");
        let latest = rows
            .iter()
            .position(|line| line.contains("reasoning row 20"))
            .expect("latest streamed row");
        assert!(latest > title && latest - title < 6);
        assert!(!rows.join("\n").contains("reasoning row 01"));
        assert!(state.transcript.is_empty());
    }

    #[test]
    fn live_thought_finishes_as_one_collapsed_summary_and_preserves_full_text() {
        let state = Arc::new(Mutex::new(ChatViewState::default()));
        let handle = ChatTui::handle(state.clone());
        handle.append_thought_delta("one", "partial", std::time::Duration::from_millis(1200));
        handle.finish_thought(
            Some("one"),
            "complete reasoning",
            std::time::Duration::from_millis(1900),
        );
        let mut state = state.lock().unwrap();
        let rendered = thought_screen(&state, 80, 24).join("\n");
        assert!(rendered.contains("Thought · 1.9s (Ctrl+T to view)"));
        assert!(!rendered.contains("Thinking ·"));
        assert!(!rendered.contains("complete reasoning"));
        assert_eq!(state.transcript.len(), 1);
        assert!(state.handle_thought_key(ctrl_t()));
        assert_eq!(
            state.thought_modal.as_ref().unwrap().lines[1..],
            vec!["complete reasoning"]
        );
    }

    #[test]
    fn live_thought_modal_refreshes_deltas_and_closes_without_cancelling_agent() {
        let state = Arc::new(Mutex::new(ChatViewState::default()));
        let handle = ChatTui::handle(state.clone());
        handle.append_thought_delta("one", "first", std::time::Duration::ZERO);
        assert!(state.lock().unwrap().handle_thought_key(ctrl_t()));
        handle.append_thought_delta("one", " second", std::time::Duration::from_secs(1));
        let mut state = state.lock().unwrap();
        assert_eq!(
            state.thought_modal.as_ref().unwrap().lines[1..],
            vec!["first second"]
        );
        assert!(state.handle_thought_key(make_key(KeyCode::Esc)));
        assert!(state.thought_modal.is_none());
        assert!(!handle.take_interrupted());
    }

    #[test]
    fn live_thought_interruption_preserves_partial_text_and_collapses_preview() {
        let state = Arc::new(Mutex::new(ChatViewState::default()));
        let handle = ChatTui::handle(state.clone());
        handle.append_thought_delta(
            "one",
            "unfinished reasoning",
            std::time::Duration::from_millis(1900),
        );
        let mut state = state.lock().unwrap();
        state.finish_live_thought();
        assert!(
            !thought_screen(&state, 80, 24)
                .join("\n")
                .contains("Thinking ·")
        );
        assert!(state.handle_thought_key(ctrl_t()));
        assert_eq!(
            state.thought_modal.as_ref().unwrap().lines[1..],
            vec!["unfinished reasoning"]
        );
        assert_eq!(state.transcript.len(), 1);
    }

    #[test]
    fn queued_message_shows_in_composer_bottom_title() {
        let mut state = ChatViewState::default();
        state.queued_count = 1;
        state.queued_preview = Some("run next test".into());
        let rendered = thought_screen(&state, 80, 24).join("\n");
        assert!(rendered.contains("Queued (1): \"run next test\" · Ctrl+D drop"));

        state.queued_count = 2;
        state.queued_preview = Some("deploy app".into());
        let rendered2 = thought_screen(&state, 80, 24).join("\n");
        assert!(rendered2.contains("Queued (2): \"deploy app\" · Ctrl+D drop"));
    }

    #[test]
    fn update_dialog_requires_explicit_selection_before_installing() {
        let (tx, _rx) = mpsc::channel();
        let mut dialog = DialogState::new_update("1.0.0", "2.0.0", tx);
        assert_eq!(
            dialog.handle_key(make_key(KeyCode::Enter)),
            Some(DialogAnswer::Choice(1))
        );
        dialog.handle_key(make_key(KeyCode::Up));
        assert_eq!(
            dialog.handle_key(make_key(KeyCode::Enter)),
            Some(DialogAnswer::Choice(0))
        );
        assert_eq!(
            dialog.handle_key(make_key(KeyCode::Esc)),
            Some(DialogAnswer::Cancel)
        );
    }

    #[test]
    fn update_dialog_can_skip_version_and_cannot_filter_away_skip() {
        let (tx, _rx) = mpsc::channel();
        let mut dialog = DialogState::new_update("1.0.0", "2.0.0", tx);
        dialog.handle_key(make_key(KeyCode::Char('x')));
        assert_eq!(
            dialog.handle_key(make_key(KeyCode::Esc)),
            Some(DialogAnswer::Cancel)
        );
        assert_eq!(dialog.handle_key(make_key(KeyCode::Char('1'))), None);
        assert_eq!(
            dialog.handle_key(make_key(KeyCode::Enter)),
            Some(DialogAnswer::Choice(0))
        );
        dialog.handle_key(make_key(KeyCode::Down));
        dialog.handle_key(make_key(KeyCode::Down));
        assert_eq!(
            dialog.handle_key(make_key(KeyCode::Enter)),
            Some(DialogAnswer::Choice(2))
        );
    }

    #[test]
    fn test_dialog_filtering() {
        let (tx, _rx) = mpsc::channel();
        let options = vec![
            "gpt-4o".to_string(),
            "claude-3-5-sonnet".to_string(),
            "gemini-2.0-flash".to_string(),
            "deepseek-r1".to_string(),
        ];
        let mut dialog = DialogState::new_choice("Pick model", "", options, None, tx);

        assert_eq!(dialog.filtered_indices(), vec![0, 1, 2, 3]);

        // Type 'g'
        dialog.handle_key(make_key(KeyCode::Char('g')));
        assert_eq!(dialog.filtered_indices(), vec![0, 2]);

        // Type 'e' -> "ge"
        dialog.handle_key(make_key(KeyCode::Char('e')));
        assert_eq!(dialog.filtered_indices(), vec![2]);

        // Backspace -> "g"
        dialog.handle_key(make_key(KeyCode::Backspace));
        assert_eq!(dialog.filtered_indices(), vec![0, 2]);

        // Esc clears filter
        dialog.handle_key(make_key(KeyCode::Esc));
        assert_eq!(dialog.filtered_indices(), vec![0, 1, 2, 3]);
    }

    #[test]
    fn test_dialog_navigation_and_numeric_shortcuts() {
        let (tx, _rx) = mpsc::channel();
        let options = vec![
            "Option 1".to_string(),
            "Option 2".to_string(),
            "Option 3".to_string(),
        ];
        let mut dialog = DialogState::new_choice("Pick one", "", options, None, tx);

        assert_eq!(dialog.selected, 0);

        dialog.handle_key(make_key(KeyCode::Down));
        assert_eq!(dialog.selected, 1);

        dialog.handle_key(make_key(KeyCode::Down));
        assert_eq!(dialog.selected, 2);

        // Clamped at bottom
        dialog.handle_key(make_key(KeyCode::Down));
        assert_eq!(dialog.selected, 2);

        dialog.handle_key(make_key(KeyCode::Up));
        assert_eq!(dialog.selected, 1);

        // Numeric shortcut: '1' selects index 0 and returns Choice(0)
        let answer = dialog.handle_key(make_key(KeyCode::Char('1')));
        assert_eq!(answer, Some(DialogAnswer::Choice(0)));

        // Numeric shortcut: '3' selects index 2 and returns Choice(2)
        let answer = dialog.handle_key(make_key(KeyCode::Char('3')));
        assert_eq!(answer, Some(DialogAnswer::Choice(2)));
    }

    #[test]
    fn test_dialog_multiselect() {
        let (tx, _rx) = mpsc::channel();
        let options = vec!["A".to_string(), "B".to_string(), "C".to_string()];
        let checked = vec![false, false, false];
        let mut dialog = DialogState::new_choice("Multi", "", options, Some(checked), tx);

        // Toggle first option with Space
        dialog.handle_key(make_key(KeyCode::Char(' ')));
        assert_eq!(dialog.checked.as_ref().unwrap(), &vec![true, false, false]);

        // Move to second option and toggle
        dialog.handle_key(make_key(KeyCode::Down));
        dialog.handle_key(make_key(KeyCode::Char(' ')));
        assert_eq!(dialog.checked.as_ref().unwrap(), &vec![true, true, false]);

        // Enter submits selected choices
        let answer = dialog.handle_key(make_key(KeyCode::Enter));
        assert_eq!(answer, Some(DialogAnswer::Choices(vec![0, 1])));
    }

    #[test]
    fn test_dialog_text_input() {
        let (tx, _rx) = mpsc::channel();
        let mut dialog = DialogState::new_text("Input", "Prompt", tx);

        dialog.handle_key(make_key(KeyCode::Char('h')));
        dialog.handle_key(make_key(KeyCode::Char('i')));
        assert_eq!(dialog.input.as_ref().unwrap(), &vec!['h', 'i']);

        dialog.handle_key(make_key(KeyCode::Backspace));
        assert_eq!(dialog.input.as_ref().unwrap(), &vec!['h']);

        let answer = dialog.handle_key(make_key(KeyCode::Enter));
        assert_eq!(answer, Some(DialogAnswer::Text("h".to_string())));
    }

    #[test]
    fn test_dialog_cancellation() {
        let (tx, _rx) = mpsc::channel();
        let mut dialog =
            DialogState::new_choice("Pick", "", vec!["A".into(), "B".into()], None, tx);

        let answer = dialog.handle_key(make_key(KeyCode::Esc));
        assert_eq!(answer, Some(DialogAnswer::Cancel));

        let ctrl_c = KeyEvent {
            code: KeyCode::Char('c'),
            modifiers: KeyModifiers::CONTROL,
            kind: KeyEventKind::Press,
            state: KeyEventState::empty(),
        };
        let answer = dialog.handle_key(ctrl_c);
        assert_eq!(answer, Some(DialogAnswer::Cancel));

        let ctrl_d = KeyEvent {
            code: KeyCode::Char('d'),
            modifiers: KeyModifiers::CONTROL,
            kind: KeyEventKind::Press,
            state: KeyEventState::empty(),
        };
        let answer = dialog.handle_key(ctrl_d);
        assert_eq!(answer, Some(DialogAnswer::Cancel));
    }

    #[test]
    fn test_transcript_entry_markdown_rendering() {
        let text = "**Bold Header**\n- Item 1\n---\n> [!TIP]\n> Pro tip here";
        let entry = TranscriptEntry::new(TranscriptRole::Assistant, text);

        // Check line synchronization
        assert_eq!(entry.rendered_lines.len(), entry.plain_lines.len());

        // Check bold rendering
        let first_rendered = &entry.rendered_lines[0];
        let has_bold = first_rendered
            .spans
            .iter()
            .any(|s| s.style.add_modifier.contains(Modifier::BOLD));
        assert!(has_bold, "First line should contain bold modifier");

        // Check divider line rendering (converted --- into sleek box drawing ─)
        let divider_line = &entry.plain_lines[2];
        assert!(
            divider_line.contains('─'),
            "--- should be converted to ─ divider: {divider_line}"
        );

        // Check TIP alert formatting
        let alert_line = &entry.plain_lines[3];
        assert!(
            alert_line.contains("│ TIP:"),
            "Alert should format as │ TIP: : {alert_line}"
        );

        // Check bullet formatting
        let bullet_line = &entry.plain_lines[1];
        assert!(
            bullet_line.contains('•'),
            "List item should format with bullet • : {bullet_line}"
        );
    }

    #[test]
    fn test_thought_modal_state_and_scrolling() {
        let mut modal = ThoughtModalState {
            elapsed_str: "3.5s".to_string(),
            lines: vec!["line 1".into(), "line 2".into(), "line 3".into()],
            scroll: 0,
        };

        assert_eq!(modal.scroll, 0);
        modal.scroll = modal.scroll.saturating_add(1);
        assert_eq!(modal.scroll, 1);
        modal.scroll = modal.scroll.saturating_sub(1);
        assert_eq!(modal.scroll, 0);
    }

    #[test]
    fn test_notice_expiration_and_clear() {
        let mut state = ChatViewState::default();
        assert_eq!(state.active_notice(), None);

        state.set_notice("Image attached");
        assert_eq!(state.active_notice(), Some("Image attached"));

        // Terminal too small warning doesn't auto-expire
        state.set_notice("Terminal too small: 40x10");
        assert_eq!(state.active_notice(), Some("Terminal too small: 40x10"));

        state.clear_notice();
        assert_eq!(state.active_notice(), None);

        // Manually test expiration logic
        state.notice = Some((
            "Old notice".into(),
            std::time::Instant::now() - std::time::Duration::from_millis(3000),
        ));
        assert_eq!(state.active_notice(), None);
        assert!(state.notice_visibility_changed_since_draw(true));
        assert!(!state.notice_visibility_changed_since_draw(false));
    }

    #[test]
    fn test_refresh_shared_transcript_preserves_tool_notice_order() {
        let temp_dir = std::env::temp_dir().join(format!(
            "mint-test-tui-sync-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let _ = std::fs::create_dir_all(&temp_dir);
        let db_path = temp_dir.join("test_memory.db");
        let memory = mint_core::MemoryStore::open(db_path);
        let workspace = temp_dir.clone();
        let chat_id = "test-session";
        let scoped = mint_core::scoped_chat_id(chat_id, Some(&workspace.to_string_lossy()));

        let mut state = ChatViewState {
            chat_id: chat_id.to_string(),
            current_dir: workspace.clone(),
            ..ChatViewState::default()
        };
        state.reload_transcript_with(&memory, chat_id, &workspace);

        // Turn 1 runs locally in CLI:
        state.push_user("List files please".into());
        state.push_notice("• Listing 2 directories...");
        state.push_notice("✓ Completed in 1.2s • 1 tool • 0 files changed");
        state.push_assistant("Found 2 files.");
        state.push_notice("─ Worked for 1s • test ──────────────────────────────");

        // In DB, TurnLease records this interaction:
        let turn_id = memory.start_turn(&scoped, "List files please").unwrap();
        memory.claim_turn(&scoped, turn_id).unwrap();
        memory
            .finish_turn(turn_id, "Found 2 files.", "test", "model", None)
            .unwrap();

        // Advance cursor as local turn completion would:
        state.advance_sync_cursor_with(&memory);

        // Refresh shared transcript:
        let redrawn = state.refresh_shared_transcript_with(&memory);
        assert!(
            !redrawn,
            "Should not redraw since changes were from local turn"
        );

        // Now simulate an external message arriving from Web/Desktop on the same session:
        let ext_id = memory.start_turn(&scoped, "Message from Web").unwrap();
        memory.claim_turn(&scoped, ext_id).unwrap();
        memory
            .finish_turn(ext_id, "Reply to Web", "test", "model", None)
            .unwrap();

        let redrawn2 = state.refresh_shared_transcript_with(&memory);
        assert!(redrawn2, "Should redraw when external turn arrives");

        // Verify transcript order:
        // Turn 1's tool notices must still be BEFORE Turn 1's assistant reply!
        // Not pushed to the very bottom!
        let texts: Vec<&str> = state.transcript.iter().map(|e| e.text.as_str()).collect();
        assert_eq!(texts.len(), 7);
        assert_eq!(texts[0], "List files please");
        assert_eq!(texts[1], "• Listing 2 directories...");
        assert_eq!(texts[2], "✓ Completed in 1.2s • 1 tool • 0 files changed");
        assert_eq!(texts[3], "Found 2 files.");
        assert_eq!(
            texts[4],
            "─ Worked for 1s • test ──────────────────────────────"
        );
        assert_eq!(texts[5], "Message from Web");
        assert_eq!(texts[6], "Reply to Web");

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_refresh_shared_transcript_external_turn_lifecycle() {
        let temp_dir = std::env::temp_dir().join(format!(
            "mint-test-tui-sync-lifecycle-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let _ = std::fs::create_dir_all(&temp_dir);
        let db_path = temp_dir.join("test_memory.db");
        let memory = mint_core::MemoryStore::open(db_path);
        let workspace = temp_dir.clone();
        let chat_id = "test-session-2";
        let scoped = mint_core::scoped_chat_id(chat_id, Some(&workspace.to_string_lossy()));

        let mut state = ChatViewState {
            chat_id: chat_id.to_string(),
            current_dir: workspace.clone(),
            ..ChatViewState::default()
        };
        state.reload_transcript_with(&memory, chat_id, &workspace);

        // Turn arrives from Web (status: queued):
        let ext_id = memory.start_turn(&scoped, "External prompt").unwrap();
        let redrawn = state.refresh_shared_transcript_with(&memory);
        assert!(redrawn);
        assert_eq!(state.transcript.len(), 2);
        assert_eq!(state.transcript[0].text, "External prompt");
        assert_eq!(state.transcript[1].text, "Queued for this session…");

        // External turn updates to running:
        memory.claim_turn(&scoped, ext_id).unwrap();
        let redrawn = state.refresh_shared_transcript_with(&memory);
        assert!(redrawn);
        assert_eq!(state.transcript.len(), 2);
        assert_eq!(state.transcript[0].text, "External prompt");
        assert_eq!(state.transcript[1].text, "Mint is responding…");

        // External turn completes:
        memory
            .finish_turn(ext_id, "External answer", "provider", "model", None)
            .unwrap();
        let redrawn = state.refresh_shared_transcript_with(&memory);
        assert!(redrawn);
        assert_eq!(state.transcript.len(), 2);
        assert_eq!(state.transcript[0].text, "External prompt");
        assert_eq!(state.transcript[1].text, "External answer");
        assert_eq!(state.transcript[1].role, TranscriptRole::Assistant);

        let _ = std::fs::remove_dir_all(&temp_dir);
    }
}

#[cfg(test)]
mod context_compaction_shimmer_tests {
    use super::*;
    #[test]
    fn full_screen_compaction_shimmers_only_the_label() {
        let text = "✦ Compacting context · 12s • Esc to interrupt";
        let first = shimmer_thinking_line(text, 0);
        let next = shimmer_thinking_line(text, 12);
        assert_ne!(first, next);
        assert_eq!(
            first.spans.last().unwrap().content,
            " · 12s • Esc to interrupt"
        );
        assert_eq!(first.spans.last(), next.spans.last());
    }
}

#[cfg(test)]
mod collapsed_paste_tests {
    use super::*;

    #[test]
    fn f2_toggles_an_edited_paste_without_folding_surrounding_instructions() {
        let mut state = ChatViewState::default();
        state.set_draft("before ".into());
        state.insert_paste("a\nb\nc\nd\ne\nf\ng");
        state.replace_composer(state.cursor..state.cursor, " after");
        let f2 = event::KeyEvent::new(event::KeyCode::F(2), event::KeyModifiers::NONE);
        state.edit_composer_key(f2);
        state.edit_composer_key(event::KeyEvent::new(
            event::KeyCode::Char('ก'),
            event::KeyModifiers::NONE,
        ));
        state.edit_composer_key(f2);
        assert_eq!(
            state.composer.iter().collect::<String>(),
            "before [Pasted Content 14 chars] after"
        );
        state.edit_composer_key(f2);
        assert_eq!(
            state.composer.iter().collect::<String>(),
            "before กa\nb\nc\nd\ne\nf\ng after"
        );
        state.edit_composer_key(f2);
        assert_eq!(state.take_prompt(), "before กa\nb\nc\nd\ne\nf\ng after");
        state.set_draft("fresh".into());
        state.edit_composer_key(f2);
        assert_eq!(state.take_prompt(), "fresh");
    }

    #[test]
    fn f2_preserves_nested_pastes_and_edits_before_the_open_block() {
        let mut state = ChatViewState::default();
        state.set_draft("before ".into());
        state.insert_paste("a\nb\nc\nd\ne\nf\ng");
        let f2 = event::KeyEvent::new(event::KeyCode::F(2), event::KeyModifiers::NONE);
        state.edit_composer_key(f2);
        state.replace_composer(0..0, "prefix ");
        state.cursor = 15;
        state.insert_paste("1\n2\n3\n4\n5\n6\n7");
        state.edit_composer_key(f2);
        assert_eq!(
            state.composer.iter().collect::<String>(),
            "prefix before [Pasted Content 26 chars]"
        );
        assert_eq!(
            state.take_prompt(),
            "prefix before a1\n2\n3\n4\n5\n6\n7\nb\nc\nd\ne\nf\ng"
        );
    }

    #[test]
    fn deleting_an_open_paste_drops_its_toggle_state() {
        let mut state = ChatViewState::default();
        state.insert_paste("a\nb\nc\nd\ne\nf\ng");
        let f2 = event::KeyEvent::new(event::KeyCode::F(2), event::KeyModifiers::NONE);
        state.edit_composer_key(f2);
        state.replace_composer(0..13, "");
        state.edit_composer_key(f2);
        assert!(state.composer.is_empty());
    }

    #[test]
    fn seven_pasted_lines_render_as_a_badge_instead_of_filling_the_composer() {
        let mut state = ChatViewState::default();
        state.insert_paste("a\nb\nc\nd\ne\nf\ng");
        let mut terminal = Terminal::new(ratatui::backend::TestBackend::new(80, 32)).unwrap();
        let mut composer = None;
        terminal
            .draw(|frame| composer = state.render(frame).composer)
            .unwrap();
        let area = composer.unwrap();
        let buffer = terminal.backend().buffer();
        let rendered: String = (area.y..area.bottom())
            .flat_map(|y| (area.x..area.right()).map(move |x| buffer[(x, y)].symbol()))
            .collect();
        assert!(rendered.contains("[Pasted Content 13 chars]"), "{rendered}");
        assert!(rendered.contains("F2"));
        assert_eq!(area.height, 3, "A collapsed paste should use one input row");
    }

    #[test]
    fn six_pasted_lines_stay_editable_inline() {
        let mut state = ChatViewState::default();
        state.insert_paste("a\r\nb\rc\nd\ne\nf");
        assert_eq!(
            state.composer.iter().collect::<String>(),
            "a\nb\nc\nd\ne\nf"
        );
    }

    #[test]
    fn autocomplete_keeps_the_adjacent_collapsed_paste_as_separate_content() {
        let mut state = ChatViewState::default();
        state.set_draft("/hel".into());
        state.insert_paste("a\nb\nc\nd\ne\nf\ng");
        state.cursor = 4;
        state.slash_selected = state
            .suggestions()
            .iter()
            .position(|(token, _)| token == "/help")
            .unwrap();
        assert!(state.apply_selected_suggestion());
        assert_eq!(state.take_prompt(), "/help a\nb\nc\nd\ne\nf\ng");
    }

    #[test]
    fn clipboard_trailing_newlines_survive_both_inline_and_collapsed_submit() {
        let mut state = ChatViewState::default();
        state.insert_paste("a\nb\nc\nd\ne\nf\n");
        assert_eq!(
            state.composer.iter().collect::<String>(),
            "a\nb\nc\nd\ne\nf\n"
        );
        assert_eq!(state.take_prompt(), "a\nb\nc\nd\ne\nf\n");
        state.insert_paste("a\nb\nc\nd\ne\nf\ng\n\n");
        assert_eq!(
            state.composer.iter().collect::<String>(),
            "[Pasted Content 15 chars]"
        );
        assert_eq!(state.take_prompt(), "a\nb\nc\nd\ne\nf\ng\n\n");
    }

    #[test]
    fn submit_expands_each_paste_without_replacing_literal_badge_text() {
        let mut state = ChatViewState::default();
        state.set_draft("literal [Pasted Content 13 chars]: ".into());
        state.insert_paste("a\nb\nc\nd\ne\nf\ng");
        state.replace_composer(state.cursor..state.cursor, " then ");
        state.insert_paste("1\n2\n3\n4\n5\n6\n7");
        assert_eq!(
            state.take_prompt(),
            "literal [Pasted Content 13 chars]: a\nb\nc\nd\ne\nf\ng then 1\n2\n3\n4\n5\n6\n7"
        );
        assert!(state.composer.is_empty());
        state.set_draft("fresh".into());
        assert_eq!(state.take_prompt(), "fresh");
    }

    #[test]
    fn opening_a_paste_reveals_its_first_line_and_edits_reach_submit() {
        let mut state = ChatViewState::default();
        state.insert_paste("a\nb\nc\nd\ne\nf\ng");
        assert!(state.open_paste());
        assert_eq!(state.cursor, 0);
        assert_eq!(
            state.composer.iter().collect::<String>(),
            "a\nb\nc\nd\ne\nf\ng"
        );
        state.replace_composer(1..1, " edited");
        assert_eq!(state.take_prompt(), "a edited\nb\nc\nd\ne\nf\ng");
    }

    #[test]
    fn deleting_part_of_a_badge_removes_only_that_paste() {
        let mut state = ChatViewState::default();
        state.set_draft("before ".into());
        state.insert_paste("a\nb\nc\nd\ne\nf\ng");
        state.replace_composer(state.cursor..state.cursor, " after");
        state.replace_composer(8..9, "");
        assert_eq!(state.take_prompt(), "before  after");
    }

    #[test]
    fn unicode_character_count_and_history_restore_keep_the_real_content() {
        let mut state = ChatViewState {
            history: vec!["older".into()],
            ..Default::default()
        };
        state.insert_paste("ก\rข\rค\rง\rจ\rฉ\rช");
        assert_eq!(
            state.composer.iter().collect::<String>(),
            "[Pasted Content 13 chars]"
        );
        state.history_previous();
        state.history_next();
        assert_eq!(state.composer_text(), "ก\nข\nค\nง\nจ\nฉ\nช");
    }
}

#[cfg(test)]
mod pasted_prompt_render_tests {
    use super::*;
    use ratatui::backend::TestBackend;

    const PROMPT: &str = "Make me a Mint mod called token-weather.\r\rWhat it should show:\r- ☀ Clear (yellow), ☁ Cloudy (cyan), ☂ Showers (blue)\r- 134.4k / 200k\r- ▁▂▃▄▅▆▇█\r- ▲ +98.3k last turn\r\rIt should update after every turn.";

    #[test]
    fn pasted_prompt_draft_renders_line_breaks_without_carriage_return_cells() {
        let mut state = ChatViewState::default();
        state.set_draft(PROMPT.into());
        state.cursor = 0;
        let mut terminal = Terminal::new(TestBackend::new(100, 40)).unwrap();
        let mut composer = None;
        terminal
            .draw(|frame| composer = state.render(frame).composer)
            .unwrap();
        let area = composer.unwrap();
        let buffer = terminal.backend().buffer();
        let rows: Vec<String> = (area.y + 1..area.bottom() - 1)
            .map(|y| {
                (area.x..area.right())
                    .map(|x| buffer[(x, y)].symbol())
                    .collect()
            })
            .collect();
        assert!(
            rows.iter().all(|row| !row.contains('\r')),
            "CR must become a line break, not a terminal cursor movement: {rows:?}"
        );
        assert!(rows[0].contains("Make me a Mint mod called token-weather."));
        assert!(rows[2].contains("What it should show:"), "{rows:?}");
        assert!(rows[3].contains("☀ Clear"));
    }

    #[test]
    fn sent_pasted_prompt_preserves_all_lines_and_weather_symbols() {
        let mut state = ChatViewState::default();
        state.push_user(PROMPT.into());
        state.push_assistant("Reading the authoring guide.");
        let mut terminal = Terminal::new(TestBackend::new(80, 40)).unwrap();
        terminal
            .draw(|frame| {
                state.render(frame);
            })
            .unwrap();
        let buffer = terminal.backend().buffer();
        let rendered: String = buffer.content.iter().map(|cell| cell.symbol()).collect();
        assert!(
            !rendered.contains('\r'),
            "Sent user text contains terminal cursor movements"
        );
        for label in [
            "token-weather.",
            "What it should show:",
            "☀ Clear",
            "134.4k / 200k",
            "▁▂▃▄▅▆▇█",
            "▲ +98.3k last turn",
            "It should update after every turn.",
        ] {
            assert!(rendered.contains(label), "Missing sent text: {label}");
        }
        assert_eq!(state.plain_transcript_lines()[2], "What it should show:");
    }

    #[test]
    fn reused_prompt_history_normalizes_crlf_and_lone_cr() {
        let mut state = ChatViewState {
            history: vec!["first\r\nsecond\rthird".into()],
            ..Default::default()
        };
        state.history_previous();
        assert_eq!(
            state.composer.iter().collect::<String>(),
            "first\nsecond\nthird"
        );
        assert_eq!(state.cursor, 18);
    }

    #[test]
    fn end_returns_scrolled_transcript_to_bottom_before_moving_draft_cursor() {
        use event::{KeyCode, KeyEvent, KeyModifiers};
        let mut state = ChatViewState::default();
        state.set_draft("first line\nsecond line".into());
        state.cursor = 2;
        state.scroll_from_bottom = 12;
        let end = KeyEvent::new(KeyCode::End, KeyModifiers::NONE);
        assert!(state.navigate_composer(end, 20));
        assert_eq!(state.scroll_from_bottom, 0);
        assert_eq!(state.cursor, 2);
        assert_eq!(state.composer_text(), "first line\nsecond line");
        assert!(state.navigate_composer(end, 20));
        assert_eq!(state.cursor, 10);
    }

    #[test]
    fn composer_navigation_uses_visual_rows_for_wrapped_and_wide_text() {
        use event::{KeyCode, KeyEvent, KeyModifiers};
        let key = |code| KeyEvent::new(code, KeyModifiers::NONE);
        let mut state = ChatViewState::default();
        state.set_draft("0123456789abcdefghijABCDEFGHIJKLMNOPQRSTUVWXYZ".into());
        assert!(state.navigate_composer(key(KeyCode::Up), 20));
        assert_eq!(state.cursor, 26);
        assert!(state.navigate_composer(key(KeyCode::Home), 20));
        assert_eq!(state.cursor, 20);
        assert!(state.navigate_composer(key(KeyCode::Down), 20));
        assert_eq!(state.cursor, 40);
        assert!(state.navigate_composer(key(KeyCode::End), 20));
        assert_eq!(state.cursor, 46);
        state.set_draft("界界\naaaa".into());
        assert!(state.navigate_composer(key(KeyCode::Up), 20));
        assert_eq!(state.cursor, 2);
        assert!(state.navigate_composer(key(KeyCode::Down), 20));
        assert_eq!(state.cursor, 7);
    }

    #[test]
    fn long_pasted_prompt_can_be_inspected_from_first_to_last_line() {
        use event::{KeyCode, KeyEvent, KeyModifiers};
        let mut state = ChatViewState::default();
        state.insert_paste(
            &(1..=12)
                .map(|i| format!("Prompt line {i:02}"))
                .collect::<Vec<_>>()
                .join("\r"),
        );
        assert!(state.edit_composer_key(KeyEvent::new(KeyCode::F(2), KeyModifiers::NONE)));
        assert_eq!(state.cursor, 0);
        state.cursor = state.composer.len();
        let mut terminal = Terminal::new(TestBackend::new(80, 32)).unwrap();
        let mut composer = None;
        terminal
            .draw(|frame| composer = state.render(frame).composer)
            .unwrap();
        let area = composer.unwrap();
        let visible = |buffer: &Buffer| -> String {
            (area.y + 1..area.bottom() - 1)
                .flat_map(|y| (area.x..area.right()).map(move |x| buffer[(x, y)].symbol()))
                .collect()
        };
        assert!(visible(terminal.backend().buffer()).contains("Prompt line 12"));
        for _ in 0..11 {
            assert!(state.navigate_composer(KeyEvent::new(KeyCode::Up, KeyModifiers::NONE), 72));
        }
        terminal
            .draw(|frame| {
                state.render(frame);
            })
            .unwrap();
        assert!(visible(terminal.backend().buffer()).contains("Prompt line 01"));
        assert!(visible(terminal.backend().buffer()).contains("Prompt line 06"));
    }
}

#[cfg(test)]
mod mod_surface_tests {
    use super::*;
    #[test]
    fn focused_mod_input_displays_the_current_value_before_editing() {
        let mut state = ChatViewState::default();
        state.mods_open = true;
        state.mods = vec![
            serde_json::json!({"id":"draft","enabled":true,"snapshot":{"ui":{"side_panel":{"type":"input","label":"Draft","action":"save","value":"existing"}}}}),
        ];
        let backend = ratatui::backend::TestBackend::new(120, 35);
        let mut terminal = ratatui::Terminal::new(backend).unwrap();
        terminal
            .draw(|frame| {
                state.render(frame);
            })
            .unwrap();
        let screen = terminal
            .backend()
            .buffer()
            .content
            .iter()
            .map(|cell| cell.symbol())
            .collect::<String>();
        assert!(screen.contains("› Draft: existing"));
    }
    #[test]
    fn terminal_shows_reload_diagnostics_while_the_old_mod_stays_active() {
        let mut state = ChatViewState::default();
        state.mods_open = true;
        state.mods = vec![
            serde_json::json!({"id":"counter","name":"Counter","enabled":true,"reloadError":"Invalid update","snapshot":{"ui":{"side_panel":{"type":"text","text":"Working revision"}}}}),
        ];
        let mut terminal = Terminal::new(ratatui::backend::TestBackend::new(120, 35)).unwrap();
        terminal
            .draw(|frame| {
                state.render(frame);
            })
            .unwrap();
        let text = terminal
            .backend()
            .buffer()
            .content
            .iter()
            .map(|cell| cell.symbol())
            .collect::<String>();
        assert!(text.contains("Reload: Invalid update"));
        assert!(text.contains("Working revision"));
    }
    #[test]
    fn terminal_viewport_tracks_docked_panel_width_and_overlay_fallback() {
        let mut state = ChatViewState::default();
        state.mods_open = true;
        let wide = state.mod_viewport(Rect::new(0, 0, 120, 35));
        assert_eq!(wide.columns, Some(78));
        assert_eq!(wide.surface.as_deref(), Some("terminal"));
        let narrow = state.mod_viewport(Rect::new(0, 0, 80, 35));
        assert_eq!(narrow.columns, Some(76));
        state.mods_open = false;
        assert_eq!(
            state.mod_viewport(Rect::new(0, 0, 120, 35)).columns,
            Some(116)
        );
    }
    #[test]
    fn terminal_weather_band_keeps_color_and_metrics_on_one_row() {
        let mut state = ChatViewState::default();
        state.mods = vec![
            serde_json::json!({"id":"weather","name":"Weather","enabled":true,"snapshot":{"ui":{"above_composer":{"type":"stack","direction":"row","children":[{"type":"text","text":"Storm","color":"magenta"},{"type":"text","text":"80% 160k / 200k"}]}}}}),
        ];
        let mut terminal = Terminal::new(ratatui::backend::TestBackend::new(120, 35)).unwrap();
        terminal
            .draw(|frame| {
                state.render(frame);
            })
            .unwrap();
        let buffer = terminal.backend().buffer();
        let mut rows = Vec::new();
        for y in 0..35 {
            rows.push(
                (0..120)
                    .map(|x| buffer[(x, y)].symbol())
                    .collect::<String>(),
            );
        }
        let row = rows
            .iter()
            .position(|r| r.contains("Storm 80% 160k / 200k"))
            .expect("Weather and counts share one row");
        let x = rows[row].find("Storm").unwrap();
        assert_eq!(
            buffer[(x as u16, row as u16)].fg,
            ratatui::style::Color::Magenta
        );
    }
    #[test]
    fn terminal_renders_mod_band_and_side_panel() {
        let mut state = ChatViewState::default();
        state.mods = vec![
            serde_json::json!({"id":"meter","name":"Meter","enabled":true,"snapshot":{"ui":{"above_composer":{"type":"text","text":"123 tokens"},"side_panel":{"type":"table","columns":["Task"],"rows":[["Write tests"]]}}}}),
        ];
        state.mods_open = true;
        let backend = ratatui::backend::TestBackend::new(120, 35);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|frame| {
                state.render(frame);
            })
            .unwrap();
        let buffer = terminal.backend().buffer();
        let text = buffer
            .content
            .iter()
            .map(|c| c.symbol())
            .collect::<String>();
        assert!(text.contains("123 tokens"));
        assert!(text.contains("Write tests"));
    }
}

#[cfg(test)]
mod swarm_panel_tests {
    use super::*;

    #[test]
    fn swarm_panel_is_hidden_when_empty() {
        let state = ChatViewState::default();
        let backend = ratatui::backend::TestBackend::new(100, 30);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|frame| {
                state.render(frame);
            })
            .unwrap();
        let text = terminal
            .backend()
            .buffer()
            .content
            .iter()
            .map(|c| c.symbol())
            .collect::<String>();
        assert!(!text.contains("Subagents"));
    }

    #[test]
    fn swarm_panel_renders_running_and_completed_subagents() {
        let mut state = ChatViewState::default();
        state.swarm_agents.push(SwarmAgentUiState {
            label: "recon".to_string(),
            profile: "explorer".to_string(),
            task: "Searching for payment webhook endpoints...".to_string(),
            current_action: None,
            status: SwarmUiStatus::Running,
            started_at: std::time::Instant::now(),
            finished_elapsed_ms: None,
            summary_preview: None,
        });
        state.swarm_agents.push(SwarmAgentUiState {
            label: "config".to_string(),
            profile: "explorer".to_string(),
            task: "Checking config files".to_string(),
            current_action: None,
            status: SwarmUiStatus::Success,
            started_at: std::time::Instant::now(),
            finished_elapsed_ms: Some(3000),
            summary_preview: Some("Checked stripe API keys in .env".to_string()),
        });

        let backend = ratatui::backend::TestBackend::new(100, 30);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|frame| {
                state.render(frame);
            })
            .unwrap();
        let text = terminal
            .backend()
            .buffer()
            .content
            .iter()
            .map(|c| c.symbol())
            .collect::<String>();
        assert!(text.contains("Running Subagents"));
        assert!(text.contains("[recon]"));
        assert!(text.contains("Searching for payment webhook endpoints..."));
        assert!(text.contains("[config]"));
        assert!(text.contains("Checked stripe API keys in .env"));
        assert!(text.contains("done 3s"));
    }

    #[test]
    fn swarm_panel_handles_more_than_5_subagents_with_overflow() {
        let mut state = ChatViewState::default();
        for i in 1..=7 {
            state.swarm_agents.push(SwarmAgentUiState {
                label: format!("agent-{}", i),
                profile: "explorer".to_string(),
                task: format!("Task description for agent {}", i),
                current_action: None,
                status: SwarmUiStatus::Running,
                started_at: std::time::Instant::now(),
                finished_elapsed_ms: None,
                summary_preview: None,
            });
        }

        let backend = ratatui::backend::TestBackend::new(100, 30);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|frame| {
                state.render(frame);
            })
            .unwrap();
        let text = terminal
            .backend()
            .buffer()
            .content
            .iter()
            .map(|c| c.symbol())
            .collect::<String>();
        assert!(text.contains("+3 more subagents..."));
    }

    #[test]
    fn swarm_panel_compact_mode_on_short_screen() {
        let mut state = ChatViewState::default();
        state.swarm_agents.push(SwarmAgentUiState {
            label: "recon".to_string(),
            profile: "explorer".to_string(),
            task: "Searching files".to_string(),
            current_action: None,
            status: SwarmUiStatus::Running,
            started_at: std::time::Instant::now(),
            finished_elapsed_ms: None,
            summary_preview: None,
        });

        let backend = ratatui::backend::TestBackend::new(80, 20);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|frame| {
                state.render(frame);
            })
            .unwrap();
        let text = terminal
            .backend()
            .buffer()
            .content
            .iter()
            .map(|c| c.symbol())
            .collect::<String>();
        assert!(text.contains("Running 1 subagent"));
    }

    #[test]
    fn swarm_agents_cleared_on_user_input_and_clear() {
        let mut state = ChatViewState::default();
        state.swarm_agents.push(SwarmAgentUiState {
            label: "recon".to_string(),
            profile: "explorer".to_string(),
            task: "Searching files".to_string(),
            current_action: None,
            status: SwarmUiStatus::Running,
            started_at: std::time::Instant::now(),
            finished_elapsed_ms: None,
            summary_preview: None,
        });
        assert_eq!(state.swarm_agents.len(), 1);

        state.push_user("hello".to_string());
        assert!(state.swarm_agents.is_empty());

        state.swarm_agents.push(SwarmAgentUiState {
            label: "recon".to_string(),
            profile: "explorer".to_string(),
            task: "Searching files".to_string(),
            current_action: None,
            status: SwarmUiStatus::Running,
            started_at: std::time::Instant::now(),
            finished_elapsed_ms: None,
            summary_preview: None,
        });
        state.clear_transcript();
        assert!(state.swarm_agents.is_empty());
    }

    #[test]
    fn record_tool_progress_updates_swarm_state() {
        let mut state = ChatViewState::default();
        let start_event = mint_core::AgentProgress::SubagentStarted {
            subagent: "worker".to_string(),
            profile: "explorer".to_string(),
            task: "Investigate database queries".to_string(),
        };
        state.record_tool_progress(&start_event);
        assert_eq!(state.swarm_agents.len(), 1);
        assert_eq!(state.swarm_agents[0].label, "worker");
        assert_eq!(state.swarm_agents[0].status, SwarmUiStatus::Running);

        let tool_start = mint_core::AgentProgress::ToolStart {
            call_id: Some("call-1".to_string()),
            action: "run_shell".to_string(),
            input: serde_json::json!({"command": "cargo test"}),
            subagent: Some("worker".to_string()),
        };
        state.record_tool_progress(&tool_start);
        assert_eq!(
            state.swarm_agents[0].current_action.as_deref(),
            Some("Running `cargo test`")
        );
        // Subagent tools must NOT be pushed to the main transcript or tools list!
        assert!(
            state.transcript.is_empty(),
            "transcript must stay clean without subagent tool spam"
        );
        assert!(
            state.tools.is_empty(),
            "tools list must not include subagent micro-tools"
        );

        let tool_end = mint_core::AgentProgress::ToolEnd {
            call_id: Some("call-1".to_string()),
            action: "run_shell".to_string(),
            input: serde_json::json!({"command": "cargo test"}),
            result: "test result ok".to_string(),
            status: Some(mint_core::integrations::mcp_result::ToolStatus::Success),
            subagent: Some("worker".to_string()),
            elapsed_ms: Some(120),
        };
        state.record_tool_progress(&tool_end);
        assert!(state.transcript.is_empty());
        assert!(state.tools.is_empty());

        let finish_event = mint_core::AgentProgress::SubagentFinished {
            subagent: "worker".to_string(),
            status: "completed".to_string(),
            elapsed_ms: 4500,
            steps: 3,
            summary_preview: "All queries optimized".to_string(),
        };
        state.record_tool_progress(&finish_event);
        assert_eq!(state.swarm_agents[0].status, SwarmUiStatus::Success);
        assert_eq!(state.swarm_agents[0].finished_elapsed_ms, Some(4500));
        assert_eq!(
            state.swarm_agents[0].summary_preview.as_deref(),
            Some("All queries optimized")
        );
        assert!(state.transcript.is_empty());
    }

    #[test]
    fn swarm_panel_renders_smoothly_in_plan_mode() {
        let mut state = ChatViewState::default();
        state.plan_mode = true;
        state.swarm_agents.push(SwarmAgentUiState {
            label: "recon".to_string(),
            profile: "explorer".to_string(),
            task: "Investigating schema for migration".to_string(),
            current_action: Some("Reading `schema.rs`".to_string()),
            status: SwarmUiStatus::Running,
            started_at: std::time::Instant::now(),
            finished_elapsed_ms: None,
            summary_preview: None,
        });

        let backend = ratatui::backend::TestBackend::new(100, 30);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|frame| {
                state.render(frame);
            })
            .unwrap();
        let text = terminal
            .backend()
            .buffer()
            .content
            .iter()
            .map(|c| c.symbol())
            .collect::<String>();
        assert!(text.contains("Running Subagents"));
        assert!(text.contains("[recon]"));
        assert!(text.contains("Reading `schema.rs`"));
        assert!(text.contains("[Plan]"));
    }

    #[test]
    fn tool_viewer_smart_jumps_to_web_search_when_sources_present() {
        use crate::interactive::tool_activity::ToolActivity;
        use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
        let mut state = ChatViewState::default();
        // Tool 0: web_search
        state.tools.push(ToolActivity {
            entry: 0,
            call_id: Some("call_web".into()),
            action: "web_search".into(),
            input: serde_json::json!({"query": "cosplay 4khd"}),
            subagent: None,
            started: std::time::Instant::now(),
            elapsed: Some(std::time::Duration::from_millis(1200)),
            result: Some("1. 4KHD Cosplay\n   URL: https://4khd.com\n   Preview text".into()),
            status: Some(mint_core::integrations::mcp_result::ToolStatus::Success),
        });
        // Tool 1: run_shell (ran afterwards)
        state.tools.push(ToolActivity {
            entry: 1,
            call_id: Some("call_shell".into()),
            action: "run_shell".into(),
            input: serde_json::json!({"command": "echo done"}),
            subagent: None,
            started: std::time::Instant::now(),
            elapsed: Some(std::time::Duration::from_millis(500)),
            result: Some("exit: 0\nstdout:\ndone\nstderr:".into()),
            status: Some(mint_core::integrations::mcp_result::ToolStatus::Success),
        });

        let make_key = |code: KeyCode, modifiers: KeyModifiers| KeyEvent::new(code, modifiers);

        // Without Sources notice, Ctrl+O opens the latest tool (index 1: run_shell)
        let key = make_key(KeyCode::Char('o'), KeyModifiers::CONTROL);
        assert!(state.handle_tool_key(key));
        assert_eq!(state.tool_viewer, Some(1));
        state.handle_tool_key(make_key(KeyCode::Esc, KeyModifiers::NONE));
        assert_eq!(state.tool_viewer, None);

        // When Sources notice is in recent transcript, Ctrl+O smart jumps to web_search (index 0)
        state.push_notice("Sources:\n1. 4KHD Cosplay\n   https://4khd.com\n  … [+10 more sources] · Ctrl+O view web_search");
        state.push_notice("─ Worked for 10s • test ──────────────────────────────");

        assert!(state.handle_tool_key(key));
        assert_eq!(state.tool_viewer, Some(0));
        assert_eq!(state.tools[state.tool_viewer.unwrap()].action, "web_search");
    }

    #[test]
    fn format_tui_web_sources_renders_with_muted_and_blue_spans_in_transcript_entry() {
        let sources: Vec<(String, String)> = (1..=10)
            .map(|i| (format!("Title {i}"), format!("https://example.com/{i}")))
            .collect();
        let formatted = crate::agent::format_tui_web_sources(&sources, 5);
        let entry = TranscriptEntry::new(TranscriptRole::Notice, formatted);

        let more_line = entry
            .rendered_lines
            .iter()
            .find(|line| {
                line.spans
                    .iter()
                    .any(|s| s.content.contains("more sources] · "))
            })
            .expect("more sources line should be present");

        let muted_span = more_line
            .spans
            .iter()
            .find(|s| s.content.contains("more sources] · "))
            .expect("muted span found");
        assert_eq!(muted_span.style.fg, Some(crate::terminal_theme::MUTED));

        let blue_span = more_line
            .spans
            .iter()
            .find(|s| s.content.contains("Ctrl+O view web_search"))
            .expect("blue span found");
        assert_eq!(blue_span.style.fg, Some(crate::terminal_theme::BLUE));
    }

    #[test]
    fn transcript_entry_lines_for_width_reflows_markdown_tables() {
        let md = "| Module | Description |\n| --- | --- |\n| chat_tui | Full-screen Ratatui interactive view |\n| slash_commands | Interactive slash command router and handlers |";
        let entry = TranscriptEntry::new_with_width(TranscriptRole::Assistant, md, 120);
        let wide_lines = entry.lines_for_width(120);
        let narrow_lines = entry.lines_for_width(35);
        assert_ne!(
            wide_lines.as_ref(),
            narrow_lines.as_ref(),
            "markdown table should reflow when terminal width narrows"
        );
    }

    #[test]
    fn resume_picker_core_supports_space_preview_and_ctrl_r_rename_state() {
        use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
        let session = mint_core::ChatSession {
            id: "s1".to_string(),
            title: "Original Title".to_string(),
            kind: "chat".to_string(),
            workspace_path: Some("/workspace/mint".to_string()),
            git_branch: Some("main".to_string()),
            main_language: Some("rust".to_string()),
            updated_at: "2026-01-01 00:00:00".to_string(),
            created_at: "2026-01-01 00:00:00".to_string(),
            message_count: 2,
            total_bytes: 128,
        };
        let (tx, _rx) = mpsc::channel();
        let mut dialog = DialogState::new_resume_picker(
            vec![session],
            "/workspace/mint".to_string(),
            Some("main".to_string()),
            "other".to_string(),
            tx,
        );
        let space = KeyEvent::new(KeyCode::Char(' '), KeyModifiers::NONE);
        assert_eq!(dialog.handle_key(space), None);
        assert!(dialog.resume_picker.as_ref().unwrap().showing_preview);
        assert_eq!(dialog.handle_key(space), None);
        assert!(!dialog.resume_picker.as_ref().unwrap().showing_preview);

        let ctrl_r = KeyEvent::new(KeyCode::Char('r'), KeyModifiers::CONTROL);
        assert_eq!(dialog.handle_key(ctrl_r), None);
        assert!(dialog.resume_picker.as_ref().unwrap().renaming);
        let esc = KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE);
        assert_eq!(dialog.handle_key(esc), None);
        assert!(!dialog.resume_picker.as_ref().unwrap().renaming);
    }
}

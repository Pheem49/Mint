use super::*;
use crate::config::AgentConfig;

#[test]
fn stringified_patch_uses_explicit_outer_target() {
    let patch = serde_json::json!({"hunks": [{"oldText": "old", "newText": "new"}]}).to_string();
    let input = parse_tool_input(
        "apply_patch",
        serde_json::json!({"path": "style.css", "patch": patch}),
    )
    .unwrap();
    let patch = input.patch.unwrap();
    assert_eq!(patch.path, PathBuf::from("style.css"));
    assert_eq!(patch.hunks.len(), 1);
}

#[tokio::test]
async fn stringified_patch_executes_only_after_approval_and_can_be_undone() {
    let root = std::env::temp_dir().join(format!("mint-patch-repro-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&root).unwrap();
    std::fs::write(root.join("style.css"), "body { color: black; }").unwrap();
    let config = MintConfig {
        allowed_read_paths: vec![root.clone()],
        allowed_write_paths: vec![root.clone()],
        blocked_paths: vec![],
        blocked_file_names: vec![],
        ..MintConfig::default()
    };
    let input = serde_json::json!({"path": "style.css", "patch": serde_json::json!({"hunks": [{"oldText": "black", "newText": "orange"}]}).to_string()});
    execute_tool_from_json(
        &root,
        &config,
        "apply_patch",
        input.clone(),
        "patch-repro",
        &mut |_| Ok(ApprovalOutcome::Denied),
    )
    .await
    .unwrap();
    assert_eq!(
        std::fs::read_to_string(root.join("style.css")).unwrap(),
        "body { color: black; }"
    );
    execute_tool_from_json(
        &root,
        &config,
        "apply_patch",
        input,
        "patch-repro",
        &mut |_| Ok(ApprovalOutcome::Approved),
    )
    .await
    .unwrap();
    assert_eq!(
        std::fs::read_to_string(root.join("style.css")).unwrap(),
        "body { color: orange; }"
    );
    crate::system::workspace_history::undo(&root, None).unwrap();
    assert_eq!(
        std::fs::read_to_string(root.join("style.css")).unwrap(),
        "body { color: black; }"
    );
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn patch_decoding_preserves_strict_types_and_rejects_ambiguous_targets() {
    let patch =
        serde_json::json!({"path": "style.css", "hunks": [{"oldText": "old", "newText": "new"}]});
    assert!(parse_tool_input("apply_patch", serde_json::json!({"patch": patch})).is_ok());
    assert!(
        parse_tool_input(
            "apply_patch",
            serde_json::json!({"path": "other.css", "patch": patch.to_string()})
        )
        .is_err()
    );
    assert!(
        parse_tool_input(
            "apply_patch",
            serde_json::json!({"patch": "{\"hunks\": []}"})
        )
        .is_err()
    );
    assert!(parse_tool_input("apply_patch", serde_json::json!({"patch": "not JSON"})).is_err());
    assert!(parse_tool_input("list_files", serde_json::json!({"limit": "100"})).is_err());
    let huge = "private-code-marker".repeat(1000);
    let error = parse_tool_input(
        "apply_patch",
        serde_json::json!({"patch": {"path": "style.css", "hunks": huge}}),
    )
    .unwrap_err()
    .to_string();
    assert!(error.len() < 600);
    assert!(!error.contains("private-code-marker"));
    let decision: AgentDecision = serde_json::from_value(serde_json::json!({"action": "apply_patch", "input": {"path": "style.css", "patch": serde_json::json!({"hunks": [{"oldText": "old", "newText": "new"}]}).to_string()}})).unwrap();
    assert_eq!(
        decision.input.patch.unwrap().path,
        PathBuf::from("style.css")
    );
}

fn test_list_call(id: &str, input: Value) -> crate::chat::ToolCall {
    crate::chat::ToolCall {
        id: id.into(),
        name: "list_files".into(),
        input,
        thought_signature: Some("signature".into()),
    }
}

#[tokio::test]
async fn native_argument_error_is_returned_and_corrected_call_can_execute() {
    let raw = serde_json::json!({"path": "Downloads/YOUTUBE_DL/playlist", "limit": "100"});
    let mut count = ToolArgumentRetries::default();
    let (decisions, errors) =
        prepare_native_tool_calls(vec![test_list_call("bad", raw.clone())], "", &mut count)
            .unwrap();
    assert!(decisions.is_empty());
    assert_eq!(errors.len(), 1);
    let mut messages = Vec::new();
    let signatures = std::collections::HashMap::from([("bad".into(), "signature".into())]);
    append_native_tool_results(&mut messages, "", &errors, &signatures, &Default::default());
    assert!(
        matches!(&messages[0].content[0], ContentBlock::ToolUse { id, input, thought_signature, .. }
            if id == "bad" && input == &raw && thought_signature.as_deref() == Some("signature"))
    );
    assert!(
        matches!(&messages[1].content[0], ContentBlock::ToolResult { tool_use_id, content, is_error }
            if tool_use_id == "bad" && *is_error && content.contains("Correct the arguments"))
    );

    let root = std::env::temp_dir().join(format!("mint-tool-recovery-{}", uuid::Uuid::new_v4()));
    let nested = root.join("Downloads/YOUTUBE_DL/playlist");
    std::fs::create_dir_all(&nested).unwrap();
    std::fs::write(nested.join("song.txt"), "song").unwrap();
    let config = MintConfig {
        allowed_read_paths: vec![root.clone()],
        blocked_paths: vec![],
        blocked_file_names: vec![],
        ..MintConfig::default()
    };
    let (decisions, errors) = prepare_native_tool_calls(
        vec![test_list_call(
            "fixed",
            serde_json::json!({"path": "Downloads/YOUTUBE_DL/playlist", "limit": 100}),
        )],
        "",
        &mut count,
    )
    .unwrap();
    assert!(errors.is_empty());
    let output = execute_tool(
        &root,
        &config,
        &decisions[0].1,
        "regression",
        &mut |_| panic!("listing must not request approval"),
        &mut |_| {},
    )
    .await
    .unwrap();
    let entries: Vec<Value> = serde_json::from_str(&output).unwrap();
    assert_eq!(entries.len(), 1);
    assert_eq!(
        entries[0]["path"],
        serde_json::json!(nested.join("song.txt"))
    );
    // Parsing the repaired call alone must not renew its budget.
    assert_eq!(count.failures["list_files"], 1);
    count.observe_completed(&[("fixed".into(), "list_files".into(), Value::Null, output).into()]);
    assert!(!count.failures.contains_key("list_files"));
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn native_invalid_calls_are_bounded_without_blocking_valid_alternatives() {
    let mut count = ToolArgumentRetries::default();
    for _ in 0..MAX_TOOL_ARGUMENT_RETRIES {
        let (decisions, errors) = prepare_native_tool_calls(
            vec![test_list_call(
                "bad",
                serde_json::json!({"path": "~/Downloads/YOUTUBE_DL", "limit": "100"}),
            )],
            "",
            &mut count,
        )
        .unwrap();
        assert!(decisions.is_empty());
        assert_eq!(errors.len(), 1);
    }
    let alternative = crate::chat::ToolCall {
        id: "alternative".into(),
        name: "run_shell".into(),
        input: serde_json::json!({"command": "ls ~/Downloads/YOUTUBE_DL"}),
        thought_signature: None,
    };
    let (decisions, errors) = prepare_native_tool_calls(vec![alternative], "", &mut count).unwrap();
    assert_eq!(decisions[0].1.action, "run_shell");
    assert!(errors.is_empty());
    assert_eq!(count.failures["list_files"], MAX_TOOL_ARGUMENT_RETRIES);
    let error = prepare_native_tool_calls(vec![test_list_call("bad", Value::Null)], "", &mut count)
        .unwrap_err();
    assert!(
        error
            .to_string()
            .contains("exhausted its 5 argument-correction retries")
    );
    // One exhausted tool must not consume another tool's corrections.
    let other = crate::chat::ToolCall {
        id: "other-bad".into(),
        name: "read_file".into(),
        input: serde_json::json!({"path": 123}),
        thought_signature: None,
    };
    let (_, errors) = prepare_native_tool_calls(vec![other], "", &mut count).unwrap();
    assert_eq!(errors.len(), 1);
    assert_eq!(count.failures["read_file"], 1);
    for result in [
        "Error: execution failed",
        "Blocked by hook",
        "Skipped duplicate",
    ] {
        count.observe_completed(&[
            ("x".into(), "list_files".into(), Value::Null, result.into()).into(),
        ]);
        assert_eq!(count.failures["list_files"], MAX_TOOL_ARGUMENT_RETRIES);
    }
    count.succeeded("list_files");
    assert_eq!(count.failures["read_file"], 1);
    assert!(!count.failures.contains_key("list_files"));
    assert!(
        prepare_native_tool_calls(vec![test_list_call("fresh", Value::Null)], "", &mut count)
            .is_ok()
    );
}

#[test]
fn native_mixed_batch_keeps_valid_calls_and_matches_each_error_to_its_id() {
    let mut count = ToolArgumentRetries::default();
    let (decisions, errors) = prepare_native_tool_calls(
        vec![
            test_list_call("bad", Value::Null),
            test_list_call("valid-1", serde_json::json!({"path": "."})),
            test_list_call("valid-2", serde_json::json!({"path": "src"})),
        ],
        "",
        &mut count,
    )
    .unwrap();
    assert_eq!(decisions.len(), 2);
    assert!(decisions_are_parallel_read_only_batch(&decisions));
    assert_eq!(errors[0].call_id, "bad");
    let mut results = errors;
    results.extend(decisions.iter().map(|(id, d)| {
        (
            id.clone(),
            d.action.clone(),
            serde_json::to_value(&d.input).unwrap(),
            "[]".into(),
        )
            .into()
    }));
    let mut messages = Vec::new();
    append_native_tool_results(
        &mut messages,
        "",
        &results,
        &Default::default(),
        &Default::default(),
    );
    assert_eq!(messages[0].content.len(), 3);
    assert_eq!(messages[1].content.len(), 3);
    assert!(
        matches!(&messages[1].content[0], ContentBlock::ToolResult { tool_use_id, is_error: true, .. } if tool_use_id == "bad")
    );
    assert!(
        matches!(&messages[1].content[1], ContentBlock::ToolResult { tool_use_id, is_error: false, .. } if tool_use_id == "valid-1")
    );
}

#[tokio::test]
async fn list_files_rejects_invalid_arguments_instead_of_listing_workspace() {
    let config = MintConfig::default();
    // A nonexistent root ensures no filesystem lookup occurs for bad input.
    let root = Path::new("/nonexistent-mint-list-files-regression");
    for input in [
        serde_json::json!({"path": "~/Downloads/YOUTUBE_DL", "limit": "100"}),
        serde_json::json!({"path": "~/Downloads/YOUTUBE_DL", "limit": -1}),
        serde_json::json!({"path": null}),
        Value::String(r#"{"path":"~/Downloads/YOUTUBE_DL""#.into()),
        Value::Null,
    ] {
        let error = execute_tool_from_json(
            root,
            &config,
            "list_files",
            input,
            "regression",
            &mut |_| panic!("invalid arguments must not request approval"),
        )
        .await
        .unwrap_err();
        assert!(
            error
                .to_string()
                .contains("invalid arguments for tool 'list_files'")
        );
    }
}

#[tokio::test]
async fn list_files_preserves_nested_paths_and_default_directory() {
    let root = std::env::temp_dir().join(format!("mint-list-files-paths-{}", std::process::id()));
    let nested = root.join("Downloads/YOUTUBE_DL/playlist");
    std::fs::create_dir_all(&nested).unwrap();
    std::fs::write(nested.join("song.txt"), "song").unwrap();
    let config = MintConfig {
        allowed_read_paths: vec![root.clone()],
        blocked_paths: vec![],
        blocked_file_names: vec![],
        ..MintConfig::default()
    };
    for input in [
        serde_json::json!({"path": "Downloads/YOUTUBE_DL/playlist", "limit": 100}),
        serde_json::json!({"path": nested}),
        serde_json::json!({}),
    ] {
        let expected = if input.get("path").is_some() {
            &nested
        } else {
            &root
        };
        let output = execute_tool_from_json(
            &root,
            &config,
            "list_files",
            input,
            "regression",
            &mut |_| panic!("listing must not request approval"),
        )
        .await
        .unwrap();
        let entries: Vec<Value> = serde_json::from_str(&output).unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(
            Path::new(entries[0]["path"].as_str().unwrap()).parent(),
            Some(expected.as_path())
        );
    }
    std::fs::remove_dir_all(root).unwrap();
}

#[tokio::test]
async fn turn_start_listener_reports_only_the_matching_chat() {
    let memory = MemoryStore::open(std::env::temp_dir().join(format!(
        "mint-turn-start-listener-{}.sqlite",
        uuid::Uuid::new_v4()
    )));
    let reported = Arc::new(std::sync::Mutex::new(Vec::new()));
    let callback_reported = reported.clone();
    with_turn_start_listener(
        "cli::one".to_string(),
        move |id| callback_reported.lock().unwrap().push(id),
        async {
            let first = TurnLease::start(&memory, "cli::one", "hello")
                .await
                .unwrap();
            let other = TurnLease::start(&memory, "cli::two", "other")
                .await
                .unwrap();
            assert_eq!(*reported.lock().unwrap(), vec![first.id]);
            drop(first);
            drop(other);
        },
    )
    .await;
}

#[test]
fn test_compact_agent_progress_cleans_deltas_and_preserves_tools() {
    let events = vec![
        AgentProgress::ToolStart {
            call_id: None,
            action: "read_file".into(),
            input: serde_json::json!({ "path": "src/main.rs" }),
            subagent: None,
        },
        AgentProgress::ThinkingDelta {
            id: "cot-1".into(),
            delta: "Reading ".into(),
            elapsed_ms: 100,
        },
        AgentProgress::ThinkingDelta {
            id: "cot-1".into(),
            delta: "the file...".into(),
            elapsed_ms: 200,
        },
        AgentProgress::ExtendedThinking {
            id: Some("cot-1".into()),
            thought: "Reading the file...".into(),
            elapsed_ms: Some(200),
        },
        AgentProgress::ToolEnd {
            elapsed_ms: None,
            call_id: None,
            status: None,
            action: "read_file".into(),
            input: serde_json::json!({ "path": "src/main.rs" }),
            result: "fn main() {}".into(),
            subagent: None,
        },
        AgentProgress::ThinkingDelta {
            id: "cot-interrupted".into(),
            delta: "Let's check ".into(),
            elapsed_ms: 300,
        },
        AgentProgress::ThinkingDelta {
            id: "cot-interrupted".into(),
            delta: "more lines".into(),
            elapsed_ms: 350,
        },
    ];

    let compacted = compact_agent_progress(&events);
    assert!(
        !compacted
            .iter()
            .any(|e| matches!(e, AgentProgress::ThinkingDelta { id, .. } if id == "cot-1"))
    );
    assert!(
        compacted
            .iter()
            .any(|e| matches!(e, AgentProgress::ToolStart { action, .. } if action == "read_file"))
    );
    assert!(
        compacted
            .iter()
            .any(|e| matches!(e, AgentProgress::ToolEnd { action, .. } if action == "read_file"))
    );
    assert!(compacted.iter().any(|e| matches!(e, AgentProgress::ExtendedThinking { id: Some(id), thought, .. } if id == "cot-1" && thought == "Reading the file...")));
    assert!(compacted.iter().any(|e| matches!(e, AgentProgress::ExtendedThinking { id: Some(id), thought, .. } if id == "cot-interrupted" && thought == "Let's check more lines")));
}

#[tokio::test]
async fn test_turn_lease_persists_agent_activity_on_complete() {
    let memory = MemoryStore::open(std::env::temp_dir().join(format!(
        "mint-turn-lease-complete-{}.sqlite",
        uuid::Uuid::new_v4()
    )));
    let mut turn = TurnLease::start(&memory, "cli::test_complete", "hello")
        .await
        .unwrap();
    let turn_id = turn.id;
    turn.activity
        .lock()
        .unwrap()
        .push(AgentProgress::ToolStart {
            call_id: None,
            action: "run_shell".into(),
            input: serde_json::json!({ "command": "ls -la" }),
            subagent: None,
        });
    turn.activity.lock().unwrap().push(AgentProgress::ToolEnd {
        elapsed_ms: None,
        call_id: None,
        status: None,
        action: "run_shell".into(),
        input: serde_json::json!({ "command": "ls -la" }),
        result: "file.txt".into(),
        subagent: None,
    });

    turn.complete_with_summary("Done listing files", "test_provider", "test_model", None)
        .unwrap();

    let rows = memory.interactions_for_chat("cli::test_complete").unwrap();
    assert_eq!(rows.len(), 1);
    let row = &rows[0];
    assert_eq!(row.id, turn_id);
    assert_eq!(row.status, "completed");
    assert_eq!(row.ai_text, "Done listing files");
    assert!(row.agent_activity.is_some());
    let activity_val = row.agent_activity.as_ref().unwrap();
    let activity_arr = activity_val.as_array().unwrap();
    assert_eq!(activity_arr.len(), 2);
    assert_eq!(activity_arr[0]["type"], "ToolStart");
    assert_eq!(activity_arr[1]["type"], "ToolEnd");
}

#[tokio::test]
async fn test_turn_lease_persists_partial_activity_on_interrupt() {
    let memory = MemoryStore::open(std::env::temp_dir().join(format!(
        "mint-turn-lease-interrupt-{}.sqlite",
        uuid::Uuid::new_v4()
    )));
    let turn = TurnLease::start(&memory, "cli::test_interrupt", "run something")
        .await
        .unwrap();
    let turn_id = turn.id;
    turn.activity
        .lock()
        .unwrap()
        .push(AgentProgress::ToolStart {
            call_id: None,
            action: "read_file".into(),
            input: serde_json::json!({ "path": "test.txt" }),
            subagent: None,
        });
    drop(turn);

    let rows = memory.interactions_for_chat("cli::test_interrupt").unwrap();
    assert_eq!(rows.len(), 1);
    let row = &rows[0];
    assert_eq!(row.id, turn_id);
    assert_eq!(row.status, "interrupted");
    assert!(row.agent_activity.is_some());
    let activity_val = row.agent_activity.as_ref().unwrap();
    let activity_arr = activity_val.as_array().unwrap();
    assert_eq!(activity_arr.len(), 1);
    assert_eq!(activity_arr[0]["type"], "ToolStart");
}

#[test]
fn extracts_a_summary_incrementally_without_matching_text_inside_other_strings() {
    let mut stream = FinishSummaryStream::default();
    let mut output = String::new();
    let mut emit = |delta: String| output.push_str(&delta);

    stream.push_tool_call(0, Some("read_file"), None, None, &mut emit);
    stream.push_tool_call(
        0,
        None,
        Some(r#"{"thought":"the \"summary\" key is data","path":"x"}"#),
        None,
        &mut emit,
    );
    assert!(stream.emitted.is_empty());

    stream.push_tool_call(1, Some("fin"), None, None, &mut emit);
    stream.push_tool_call(1, Some("finish"), None, None, &mut emit);
    stream.push_tool_call(1, None, Some(r#"{"summary":"สวัส"#), None, &mut emit);
    stream.push_tool_call(
        1,
        None,
        Some(r#"ดี\nโลก","verification":""}"#),
        None,
        &mut emit,
    );

    assert_eq!(output, "สวัสดี\nโลก");
}

#[test]
fn extracts_json_prompt_summary_as_chunks_arrive() {
    let mut stream = FinishSummaryStream::default();
    let mut output = String::new();
    let mut emit = |delta: String| output.push_str(&delta);
    stream.push_json_prompt(
        r#"{"thought":"done","action":"finish","input":{"summary":"Hello"#,
        &mut emit,
    );
    assert_eq!(stream.emitted, "Hello");
    stream.push_json_prompt(r#" world"}}"#, &mut emit);
    assert_eq!(output, "Hello world");
}

#[test]
fn direct_native_text_is_not_sent_again_as_a_completed_summary() {
    assert_eq!(
        unstreamed_summary_remainder("A streamed answer.", "", "A streamed answer."),
        ""
    );
}

#[test]
fn direct_native_text_appends_only_the_unstreamed_suffix() {
    assert_eq!(
        unstreamed_summary_remainder("A streamed answer, continued.", "", "A streamed answer"),
        ", continued."
    );
}

#[test]
fn streamed_finish_summary_takes_precedence_over_direct_text() {
    assert_eq!(
        unstreamed_summary_remainder("Final answer.", "Final", "unrelated preamble"),
        " answer."
    );
}

#[test]
fn truncate_for_context_leaves_short_text_untouched() {
    assert_eq!(truncate_for_context("hello", 400), "hello");
}

#[test]
fn truncate_for_context_truncates_long_text_with_a_marker() {
    let long = "a".repeat(500);
    let result = truncate_for_context(&long, 400);
    assert_eq!(
        result.chars().count(),
        400 + "... [truncated]".chars().count()
    );
    assert!(result.starts_with(&"a".repeat(400)));
    assert!(result.ends_with("... [truncated]"));
}

#[test]
fn truncate_for_context_never_splits_a_multibyte_char() {
    // Thai text is multi-byte UTF-8; a byte-index slice here would panic
    // or produce invalid UTF-8 if it landed mid-character.
    let thai = "สวัสดี".repeat(200);
    let result = truncate_for_context(&thai, 5);
    assert_eq!(
        result.chars().count(),
        5 + "... [truncated]".chars().count()
    );
}

#[test]
fn slugify_lowercases_and_collapses_separators() {
    assert_eq!(
        slugify("Retry Flaky Playwright Tests!!"),
        "retry-flaky-playwright-tests"
    );
    assert_eq!(
        slugify("  leading/trailing --dashes--  "),
        "leading-trailing-dashes"
    );
    assert_eq!(slugify("already-a-slug"), "already-a-slug");
    assert_eq!(slugify("***"), "");
}

#[test]
fn skill_revision_defaults_to_zero_without_a_revisions_line() {
    assert_eq!(skill_revision("no frontmatter at all"), 0);
    assert_eq!(skill_revision("---\ndescription: a skill\n---\nbody"), 0);
}

#[test]
fn skill_revision_reads_back_what_set_skill_revision_wrote() {
    let original = "---\ndescription: a skill\n---\nstep-by-step body";
    let bumped = set_skill_revision(original, 1);
    assert_eq!(skill_revision(&bumped), 1);
    assert!(
        bumped.contains("description: a skill"),
        "must preserve existing frontmatter fields: {bumped}"
    );
    assert!(
        bumped.contains("step-by-step body"),
        "must preserve the body: {bumped}"
    );

    // Refining again must replace the old count, not accumulate a
    // second `revisions:` line alongside it.
    let bumped_again = set_skill_revision(&bumped, 2);
    assert_eq!(skill_revision(&bumped_again), 2);
    assert_eq!(
        bumped_again.matches("revisions:").count(),
        1,
        "expected exactly one revisions line, got: {bumped_again}"
    );
}

#[test]
fn set_skill_revision_adds_a_frontmatter_block_when_content_has_none() {
    let result = set_skill_revision("just a body, no frontmatter", 1);
    assert_eq!(skill_revision(&result), 1);
    assert!(result.contains("just a body, no frontmatter"));
}

#[test]
fn existing_workspace_skill_bodies_reports_none_yet_for_a_fresh_workspace() {
    let root = std::env::temp_dir().join("mint-memory-skill-test-no-skills-yet");
    let _ = std::fs::remove_dir_all(&root);
    assert_eq!(existing_workspace_skill_bodies(&root), "(none yet)");
}

#[test]
fn existing_workspace_skill_bodies_includes_full_content_by_slug() {
    let root = std::env::temp_dir().join("mint-memory-skill-test-existing-bodies");
    let _ = std::fs::remove_dir_all(&root);
    let skill_dir = root
        .join(".agents")
        .join("skills")
        .join("retry-flaky-tests");
    std::fs::create_dir_all(&skill_dir).unwrap();
    std::fs::write(
        skill_dir.join("SKILL.md"),
        "---\ndescription: retry flaky tests\n---\nrun with --retries=3",
    )
    .unwrap();

    let bodies = existing_workspace_skill_bodies(&root);
    assert!(bodies.contains("--- retry-flaky-tests ---"));
    assert!(bodies.contains("run with --retries=3"));

    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn skill_worthy_requires_enough_steps_and_substantive_work() {
    let mut counts = BTreeMap::new();
    counts.insert("read_file:foo.rs".to_string(), 1);
    counts.insert("apply_patch:foo.rs".to_string(), 1);

    // Too few steps, even with a substantive action.
    assert!(!looks_skill_worthy(2, &counts));
    // Enough steps, has a substantive action.
    assert!(looks_skill_worthy(3, &counts));

    let mut read_only = BTreeMap::new();
    read_only.insert("read_file:foo.rs".to_string(), 5);
    read_only.insert("web_search::rust patterns".to_string(), 2);
    // Enough steps, but nothing substantive happened.
    assert!(!looks_skill_worthy(5, &read_only));
}

#[test]
fn shell_result_failed_reads_the_exit_line() {
    assert!(!shell_result_failed(
        "exit: 0\nmode: normal\nsandboxed: true\nstdout:\nok\nstderr:\n"
    ));
    assert!(shell_result_failed(
        "exit: 1\nmode: normal\nsandboxed: true\nstdout:\nfail\nstderr:\n"
    ));
    // A killed/unknown-status process isn't treated as a confirmed failure.
    assert!(shell_result_failed(
        "exit: unknown\nmode: normal\nsandboxed: true\nstdout:\nstderr:\n"
    ));
}

#[test]
fn shell_result_failed_catches_a_later_command_failing_in_a_multi_command_verify() {
    // A multi-command `verify` joins each command's own `run_shell` block with
    // "\n\n" — the first command here passes, the second fails, and the scan
    // must not stop after the first "exit: " line it finds.
    let joined = "exit: 0\nmode: normal\nsandboxed: true\nstdout:\ncargo check ok\nstderr:\n\n\
                       exit: 1\nmode: normal\nsandboxed: true\nstdout:\n\nstderr:\n2 tests failed";
    assert!(shell_result_failed(joined));
}

#[test]
fn unverified_modification_requires_a_verify_call_after_the_last_edit() {
    // No edit at all this run — nothing to verify.
    assert!(!unverified_modification(None, None, ""));
    // Edited, never verified, no explanation given.
    assert!(unverified_modification(Some(2), None, ""));
    // Edited, verified *before* the edit (stale) — still unverified.
    assert!(unverified_modification(Some(2), Some(1), ""));
    // Edited, verified after — satisfied regardless of what verify found.
    assert!(!unverified_modification(Some(2), Some(3), ""));
    // An explanation cannot promote an unchecked edit to verified.
    assert!(unverified_modification(
        Some(2),
        None,
        "Docs-only change, no test suite."
    ));
}

#[test]
fn unacknowledged_verify_failure_requires_the_agent_to_address_a_real_failure() {
    // No verify ran yet, or it passed — nothing to block on.
    assert!(!unacknowledged_verify_failure(None, ""));
    assert!(!unacknowledged_verify_failure(Some(false), ""));
    // Verify failed and the agent said nothing about it — this is the
    // "claims success while a check is actually failing" case.
    assert!(unacknowledged_verify_failure(Some(true), ""));
    assert!(unacknowledged_verify_failure(Some(true), "n/a"));
    // Even an explanation of pre-existing failures does not make them pass.
    assert!(unacknowledged_verify_failure(
        Some(true),
        "2 pre-existing test failures unrelated to this change; see notes."
    ));
}

#[test]
fn preserves_request_without_history() {
    let store = MemoryStore::open(
        std::env::temp_dir().join(format!("mint-orchestrator-{}.sqlite", std::process::id())),
    );
    let request = ChatRequest {
        message: "hello".into(),
        system_instruction: "system".into(),
        chat_id: None,
        image_data_uri: None,
        audio_data_uri: None,
        video_data_uri: None,
        document_attachment: None,
        workspace_path: None,
        agent_id: None,
        plan_mode: false,
        pinned_mcp_server: None,
        messages: None,
        tools: None,
        temperature: None,
        ..Default::default()
    };
    let config = MintConfig::default();
    assert!(
        enrich_request(&config, &store, &request)
            .unwrap()
            .system_instruction
            .starts_with("system")
    );
}

fn insert_completed_turn(
    store: &MemoryStore,
    chat_id: &str,
    user_text: &str,
    ai_text: &str,
) -> i64 {
    let turn = store.start_turn(chat_id, user_text).unwrap();
    store.claim_turn(chat_id, turn).unwrap();
    store
        .finish_turn(turn, ai_text, "test-provider", "test-model", None)
        .unwrap();
    turn
}

#[test]
fn build_conversation_history_messages_empty_on_fresh_session() {
    let db_path =
        std::env::temp_dir().join(format!("mint-test-fresh-{}.sqlite", uuid::Uuid::new_v4()));
    let store = MemoryStore::open(&db_path);
    let messages = build_conversation_history_messages(&store, "fresh-session-123", 15, 4000);
    assert!(messages.is_empty());
    let _ = std::fs::remove_file(db_path);
}

#[test]
fn build_conversation_history_messages_preserves_order_and_roles() {
    let db_path =
        std::env::temp_dir().join(format!("mint-test-order-{}.sqlite", uuid::Uuid::new_v4()));
    let store = MemoryStore::open(&db_path);
    let chat_id = "test-chat-multi-turn";

    // Insert Turn 1
    insert_completed_turn(
        &store,
        chat_id,
        "ถามชื่อหน่อยดิ แล้วก็ถามผลไม้ที่ชอบหน่อย",
        "ได้เลยครับ! คุณชื่ออะไร และชอบทานผลไม้อะไรเป็นพิเศษครับ?",
    );

    // Insert Turn 2
    insert_completed_turn(
        &store,
        chat_id,
        "ฉันชื่อภีม ชอบกินมะม่วง",
        "สวัสดีครับคุณภีม ยินดีที่ได้รู้จักครับ! มะม่วงหวานอร่อยมากครับ",
    );

    let messages = build_conversation_history_messages(&store, chat_id, 15, 4000);
    assert_eq!(messages.len(), 4);

    // Verify chronological order and role alternation
    assert_eq!(messages[0].role, ChatRole::User);
    assert_eq!(
        messages[0].content,
        vec![ContentBlock::Text {
            text: "ถามชื่อหน่อยดิ แล้วก็ถามผลไม้ที่ชอบหน่อย".to_string()
        }]
    );

    assert_eq!(messages[1].role, ChatRole::Assistant);
    assert_eq!(
        messages[1].content,
        vec![ContentBlock::Text {
            text: "ได้เลยครับ! คุณชื่ออะไร และชอบทานผลไม้อะไรเป็นพิเศษครับ?".to_string()
        }]
    );

    assert_eq!(messages[2].role, ChatRole::User);
    assert_eq!(
        messages[2].content,
        vec![ContentBlock::Text {
            text: "ฉันชื่อภีม ชอบกินมะม่วง".to_string()
        }]
    );

    assert_eq!(messages[3].role, ChatRole::Assistant);
    assert_eq!(
        messages[3].content,
        vec![ContentBlock::Text {
            text: "สวัสดีครับคุณภีม ยินดีที่ได้รู้จักครับ! มะม่วงหวานอร่อยมากครับ".to_string()
        }]
    );

    let _ = std::fs::remove_file(db_path);
}

#[test]
fn build_conversation_history_messages_handles_empty_ai_text_fallback() {
    let db_path = std::env::temp_dir().join(format!(
        "mint-test-fallback-{}.sqlite",
        uuid::Uuid::new_v4()
    ));
    let store = MemoryStore::open(&db_path);
    let chat_id = "test-chat-empty-ai";

    insert_completed_turn(&store, chat_id, "run silent task", "   ");

    let messages = build_conversation_history_messages(&store, chat_id, 15, 4000);
    assert_eq!(messages.len(), 2);
    assert_eq!(messages[0].role, ChatRole::User);
    assert_eq!(messages[1].role, ChatRole::Assistant);
    assert_eq!(
        messages[1].content,
        vec![ContentBlock::Text {
            text: "(completed)".to_string()
        }]
    );

    let _ = std::fs::remove_file(db_path);
}

#[test]
fn build_conversation_history_messages_truncation_preserves_multibyte() {
    let db_path =
        std::env::temp_dir().join(format!("mint-test-trunc-{}.sqlite", uuid::Uuid::new_v4()));
    let store = MemoryStore::open(&db_path);
    let chat_id = "test-chat-trunc";

    let long_thai = "สวัสดีชาวโลก ".repeat(400); // > 4,000 characters
    insert_completed_turn(&store, chat_id, &long_thai, "รับทราบครับ");

    let messages = build_conversation_history_messages(&store, chat_id, 15, 4000);
    assert_eq!(messages.len(), 2);
    if let ContentBlock::Text { text } = &messages[0].content[0] {
        assert!(text.ends_with("... [truncated]"));
        assert_eq!(
            text.chars().count(),
            4000 + "... [truncated]".chars().count()
        );
    } else {
        panic!("expected ContentBlock::Text");
    }

    let _ = std::fs::remove_file(db_path);
}

#[test]
fn render_conversation_transcript_formats_cleanly() {
    let db_path = std::env::temp_dir().join(format!(
        "mint-test-transcript-{}.sqlite",
        uuid::Uuid::new_v4()
    ));
    let store = MemoryStore::open(&db_path);
    let chat_id = "test-chat-transcript";

    insert_completed_turn(&store, chat_id, "คำถามข้อ 1", "คำตอบข้อ 1");

    let transcript = render_conversation_transcript(&store, chat_id, 15, 4000).unwrap();
    assert!(transcript.contains("User: คำถามข้อ 1"));
    assert!(transcript.contains("Assistant: คำตอบข้อ 1"));

    let _ = std::fs::remove_file(db_path);
}

#[test]
fn enrich_request_populates_messages_and_omits_duplicate_transcript() {
    let db_path =
        std::env::temp_dir().join(format!("mint-test-enrich-{}.sqlite", uuid::Uuid::new_v4()));
    let store = MemoryStore::open(&db_path);
    let chat_id = "test-chat-enrich";

    insert_completed_turn(&store, chat_id, "คุยรอบแรก", "ตอบรอบแรก");

    let request = ChatRequest {
        message: "คุยรอบสอง".into(),
        system_instruction: "You are a helpful assistant.".into(),
        chat_id: Some(chat_id.to_string()),
        image_data_uri: None,
        audio_data_uri: None,
        video_data_uri: None,
        document_attachment: None,
        workspace_path: None,
        agent_id: None,
        plan_mode: false,
        pinned_mcp_server: None,
        messages: None,
        tools: None,
        temperature: None,
        ..Default::default()
    };
    let config = MintConfig::default();
    let enriched = enrich_request(&config, &store, &request).unwrap();

    // Verify structured messages populated: 2 historical + 1 current = 3 messages
    assert!(enriched.messages.is_some());
    let msgs = enriched.messages.unwrap();
    assert_eq!(msgs.len(), 3);
    assert_eq!(msgs[0].role, ChatRole::User);
    assert_eq!(
        msgs[0].content,
        vec![ContentBlock::Text {
            text: "คุยรอบแรก".into()
        }]
    );
    assert_eq!(msgs[1].role, ChatRole::Assistant);
    assert_eq!(
        msgs[1].content,
        vec![ContentBlock::Text {
            text: "ตอบรอบแรก".into()
        }]
    );
    assert_eq!(msgs[2].role, ChatRole::User);
    assert_eq!(
        msgs[2].content,
        vec![ContentBlock::Text {
            text: "คุยรอบสอง".into()
        }]
    );

    // Verify system instruction does NOT duplicate "Recent conversation context:"
    assert!(
        !enriched
            .system_instruction
            .contains("Recent conversation context:")
    );

    let _ = std::fs::remove_file(db_path);
}

#[test]
fn agent_decision_allows_null_input() {
    let decision = parse_decision(r#"{"thought":"done","action":"finish","input":null}"#)
        .expect("null input should parse as default input");

    assert_eq!(decision.action, "finish");
    assert!(decision.input.summary.is_empty());
}

#[test]
fn agent_decision_allows_missing_input() {
    let decision = parse_decision(r#"{"thought":"done","action":"finish"}"#)
        .expect("missing input should parse as default input");

    assert_eq!(decision.action, "finish");
    assert!(decision.input.summary.is_empty());
}

#[test]
fn shorthand_finish_allows_null_or_missing_finish() {
    let decision =
        parse_decision(r#"{"thought":"done","finish":null}"#).expect("null finish should parse");
    assert_eq!(decision.action, "finish");
    assert!(decision.input.summary.is_empty());

    let decision = parse_decision(r#"{"thought":"done"}"#).expect("missing finish should parse");
    assert_eq!(decision.action, "finish");
    assert!(decision.input.summary.is_empty());
}

#[test]
fn shorthand_finish_allows_string_finish_as_summary() {
    let decision = parse_decision(r#"{"thought":"done","finish":"all done!"}"#)
        .expect("string finish should parse as summary");
    assert_eq!(decision.action, "finish");
    assert_eq!(decision.input.summary, "all done!");
}

#[test]
fn write_file_policy_rejects_existing_workspace_file() {
    let root = std::env::temp_dir().join(format!("mint-write-file-policy-{}", std::process::id()));
    std::fs::create_dir_all(&root).unwrap();
    let target = root.join("existing.txt");
    std::fs::write(&target, "already here").unwrap();
    let config = MintConfig {
        allowed_read_paths: vec![root.clone()],
        allowed_write_paths: vec![root.clone()],
        blocked_paths: vec![],
        blocked_file_names: vec![],
        ..MintConfig::default()
    };

    let result = validate_new_workspace_file(&root, &config, Path::new("existing.txt"));

    assert!(
        matches!(result, Err(OrchestrationError::Agent(message)) if message.contains("Use apply_patch"))
    );
    let _ = std::fs::remove_file(target);
    let _ = std::fs::remove_dir(root);
}

#[test]
fn agent_list_files_includes_directories() {
    let root = std::env::temp_dir().join(format!(
        "mint-agent-list-directories-{}",
        std::process::id()
    ));
    std::fs::create_dir_all(root.join("Bunny Girl")).unwrap();
    std::fs::write(root.join("note.txt"), "hello").unwrap();
    let config = MintConfig {
        allowed_read_paths: vec![root.clone()],
        allowed_write_paths: vec![root.clone()],
        blocked_paths: vec![],
        blocked_file_names: vec![],
        ..MintConfig::default()
    };

    let entries = list_directory_entries(&root, 100, &config).unwrap();

    assert!(entries.iter().any(|entry| {
        entry.name == "Bunny Girl" && entry.kind == "directory" && entry.size.is_none()
    }));
    assert!(entries.iter().any(|entry| {
        entry.name == "note.txt" && entry.kind == "file" && entry.size == Some(5)
    }));
    let _ = std::fs::remove_file(root.join("note.txt"));
    let _ = std::fs::remove_dir(root.join("Bunny Girl"));
    let _ = std::fs::remove_dir(root);
}

#[test]
fn grep_is_classified_as_read_only() {
    let classification = classify_shell_command("ls ~/Downloads | grep -F \"Bunny Girl\"");

    assert_eq!(classification.mode.as_str(), "readOnly");
}

#[test]
fn test_resolve_agent_config() {
    let mut config = MintConfig::default();
    config.enable_agent_collaboration = true;
    config.agents = vec![
        AgentConfig {
            id: "planner".into(),
            name: "Planner".into(),
            provider: "openai".into(),
            model: "gpt-4o".into(),
            api_key: Some("test-key".into()),
            system_instruction: "Planner instruction".into(),
            enabled: true,
        },
        AgentConfig {
            id: "coder".into(),
            name: "Coder".into(),
            provider: "gemini".into(),
            model: "gemini-2.5-flash".into(),
            api_key: None,
            system_instruction: "Coder instruction".into(),
            enabled: true,
        },
    ];

    let (cfg, instr, name, model) = resolve_agent_config(&config, Some("planner"), &[]);
    assert_eq!(cfg.ai_provider, "openai");
    assert_eq!(cfg.openai_model, "gpt-4o");
    assert_eq!(cfg.openai_api_key, "test-key");
    assert_eq!(instr, "Planner instruction");
    assert_eq!(name, Some("Planner".to_string()));
    assert_eq!(model, Some("gpt-4o".to_string()));

    let (cfg, instr, name, _model) = resolve_agent_config(&config, None, &[]);
    assert_eq!(cfg.ai_provider, "openai");
    assert_eq!(instr, "Planner instruction");
    assert_eq!(name, Some("Planner".to_string()));

    let trajectory = vec!["Step 1:\n- Action: create_plan\n- Observation: all planned".to_string()];
    let (cfg, instr, name, model) = resolve_agent_config(&config, None, &trajectory);
    assert_eq!(cfg.ai_provider, "gemini");
    assert_eq!(instr, "Coder instruction");
    assert_eq!(name, Some("Coder".to_string()));
    assert_eq!(model, Some("gemini-2.5-flash".to_string()));
}

fn step_pair(index: usize) -> [ChatMessage; 2] {
    [
        ChatMessage {
            role: ChatRole::Assistant,
            content: vec![ContentBlock::ToolUse {
                id: format!("call_{index}"),
                name: "read_file".into(),
                input: serde_json::json!({ "path": format!("file{index}.rs") }),
                thought_signature: None,
            }],
        },
        ChatMessage {
            role: ChatRole::Tool,
            content: vec![ContentBlock::ToolResult {
                tool_use_id: format!("call_{index}"),
                content: format!("contents of file{index}.rs"),
                is_error: false,
            }],
        },
    ]
}

fn native_messages_with_steps(step_count: usize) -> Vec<ChatMessage> {
    let mut messages = vec![ChatMessage::text(ChatRole::User, "do the task")];
    for i in 0..step_count {
        messages.extend(step_pair(i));
    }
    messages
}

#[tokio::test]
async fn compact_native_messages_is_a_noop_when_history_is_short() {
    // COMPACTION_KEEP_RECENT_STEPS = 3, so exactly 3 step-pairs is nothing to compact yet.
    let messages = native_messages_with_steps(3);
    let config = MintConfig::default();
    let result = compact_native_messages(&config, &messages).await.unwrap();
    assert!(result.is_none());
}

#[tokio::test]
async fn compact_native_messages_is_a_noop_for_empty_history() {
    let config = MintConfig::default();
    let result = compact_native_messages(&config, &[]).await.unwrap();
    assert!(result.is_none());
}

#[test]
fn render_messages_as_text_includes_tool_calls_and_results() {
    let messages = native_messages_with_steps(1);
    let rendered = render_messages_as_text(&messages);
    assert!(rendered.contains("Called read_file with"));
    assert!(rendered.contains("Result: contents of file0.rs"));
    assert!(rendered.contains("do the task"));
}

#[test]
fn no_modification_means_finish_never_needs_verification() {
    // A pure Q&A run that never touched apply_patch/write_file can finish
    // immediately, regardless of what (if anything) is in `verification`.
    assert!(!unverified_modification(None, None, ""));
    assert!(!unverified_modification(None, Some(1), ""));
}

#[test]
fn modification_without_any_verify_call_blocks_finish() {
    assert!(unverified_modification(Some(2), None, ""));
}

#[test]
fn verify_called_before_the_modification_does_not_count() {
    // Editing again after the last verify invalidates that earlier check.
    assert!(unverified_modification(Some(3), Some(1), ""));
}

#[test]
fn verify_called_after_the_modification_satisfies_the_gate() {
    assert!(!unverified_modification(Some(2), Some(3), ""));
}

#[test]
fn verify_on_the_same_step_as_the_modification_satisfies_the_gate() {
    // Both markers can land on the same step if a single turn issues
    // multiple tool calls (apply_patch then verify back to back).
    assert!(!unverified_modification(Some(2), Some(2), ""));
}

#[test]
fn explicit_written_justification_does_not_verify_an_unchecked_edit() {
    assert!(unverified_modification(
        Some(2),
        None,
        "Documentation-only change; no test suite in this repo."
    ));
}

#[test]
fn placeholder_verification_text_does_not_satisfy_the_gate() {
    // Mirrors `meaningful_verification`'s own placeholder filter — "n/a" etc.
    // must not be treated as a real justification.
    assert!(unverified_modification(Some(2), None, "n/a"));
    assert!(unverified_modification(Some(2), None, "not run"));
}

fn decision_with_action(action: &str) -> (String, AgentDecision) {
    (
        "call_0".to_string(),
        AgentDecision {
            thought: String::new(),
            action: action.to_string(),
            input: AgentInput::default(),
        },
    )
}

#[test]
fn empty_decisions_never_qualify_for_the_parallel_batch() {
    assert!(!decisions_are_parallel_read_only_batch(&[]));
}

#[test]
fn two_or_more_read_only_decisions_qualify_for_the_parallel_batch() {
    let decisions = vec![
        decision_with_action("read_file"),
        decision_with_action("search_code"),
    ];
    assert!(decisions_are_parallel_read_only_batch(&decisions));

    let decisions = vec![
        decision_with_action("read_file"),
        decision_with_action("web_search"),
        decision_with_action("git_diff"),
    ];
    assert!(decisions_are_parallel_read_only_batch(&decisions));
}

#[test]
fn a_lone_read_only_decision_stays_sequential() {
    let decisions = vec![decision_with_action("read_file")];
    assert!(!decisions_are_parallel_read_only_batch(&decisions));
}

#[test]
fn read_only_mixed_with_mutating_action_stays_sequential() {
    let decisions = vec![
        decision_with_action("read_file"),
        decision_with_action("write_file"),
    ];
    assert!(!decisions_are_parallel_read_only_batch(&decisions));

    let decisions = vec![
        decision_with_action("read_file"),
        decision_with_action("run_shell"),
    ];
    assert!(!decisions_are_parallel_read_only_batch(&decisions));
}

#[test]
fn rebuild_observation_reuses_buffer_and_formats_history() {
    let root = Path::new("/workspace/test-project");
    let task = "Fix the login bug";
    let step1 = format_trajectory_step(1, "Inspecting files", "list_files", "src/main.rs");
    let step2 = format_trajectory_step(2, "Reading code", "read_file", "fn main() {}");
    let trajectory = vec![step1, step2];

    let mut buffer = String::new();
    rebuild_observation(task, root, &trajectory, &mut buffer);

    assert!(buffer.contains("Task: Fix the login bug"));
    assert!(buffer.contains("Workspace: /workspace/test-project"));
    assert!(buffer.contains("Step 1:\n- Thought: Inspecting files"));
    assert!(buffer.contains("Step 2:\n- Thought: Reading code"));
    assert!(buffer.ends_with(
        "Proceed to the next step. If you have completed the task, use the 'finish' action."
    ));

    // Ensure buffer can be reused cleanly for subsequent steps
    let step3 = format_trajectory_step(3, "All done", "finish", "Fixed");
    let mut extended_trajectory = trajectory;
    extended_trajectory.push(step3);
    rebuild_observation(task, root, &extended_trajectory, &mut buffer);

    assert!(buffer.contains("Step 3:\n- Thought: All done"));
}

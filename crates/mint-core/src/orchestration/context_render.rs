use std::path::Path;

use super::*;

/// Renders a slice of `native_messages` back into readable text for the
/// compaction summarizer prompt. Self-contained to `ChatMessage`/`ContentBlock`
/// rather than reusing the parallel `trajectory: Vec<String>` log, since that
/// log gets one entry per *tool call* while `native_messages` gets one
/// Assistant/Tool pair per *step* (a step can batch multiple tool calls) —
/// keeping the two aligned would need extra bookkeeping for no real benefit.
pub(super) fn render_messages_as_text(messages: &[ChatMessage]) -> String {
    messages
        .iter()
        .map(|message| {
            let rendered = message
                .content
                .iter()
                .map(|block| match block {
                    ContentBlock::Text { text } => text.clone(),
                    ContentBlock::ToolUse { name, input, .. } => {
                        format!("Called {name} with {input}")
                    }
                    ContentBlock::ToolResult { content, .. } => {
                        let truncated = super::truncation::head_tail_truncate(
                            content,
                            super::truncation::TruncationConfig::COMPACT,
                        );
                        format!("Result: {truncated}")
                    }
                    ContentBlock::Image { .. } => "[image]".to_string(),
                    ContentBlock::Audio { .. } => "[audio]".to_string(),
                    ContentBlock::Video { .. } => "[video]".to_string(),
                })
                .collect::<Vec<_>>()
                .join("\n");
            format!("{:?}: {rendered}", message.role)
        })
        .collect::<Vec<_>>()
        .join("\n\n")
}

/// Compacts the older portion of `native_messages` into a single synthetic
/// step-pair summary once the conversation is approaching the model's context
/// window, keeping the last `COMPACTION_KEEP_RECENT_STEPS` step-pairs verbatim.
///
/// `messages[0]` is always the initial task/observation message, and every
/// message after it is a strict repeating `[Assistant, Tool]` pair — one pair
/// per outer agent-loop step, even when that step batched multiple tool calls
/// (see the loop body). Cutting only on pair boundaries means the result
/// always preserves valid role alternation for every provider, with no
/// special-casing needed elsewhere.
///
/// `Ok(None)` means there was nothing worth compacting yet (too little history)
/// — not a failure, just a no-op.
///
/// If LLM-based summarization fails (e.g. timeout, rate limit, provider error),
/// this function falls back to deterministic structural truncation via
/// [`hard_truncate_native_messages`] to guarantee that context is reduced and
/// the agent loop does not crash from context overflow.
pub(super) async fn compact_native_messages(
    config: &MintConfig,
    messages: &[ChatMessage],
    history_prefix_len: usize,
) -> Result<Option<(Vec<ChatMessage>, Option<String>)>, ChatError> {
    let step_start = history_prefix_len + 1;
    if messages.len() <= step_start {
        return Ok(None);
    }
    let step_pairs = (messages.len() - step_start) / 2;
    if step_pairs <= COMPACTION_KEEP_RECENT_STEPS {
        return Ok(None);
    }
    let compact_pairs = step_pairs - COMPACTION_KEEP_RECENT_STEPS;
    let compact_message_count = compact_pairs * 2;
    let compacted_range = &messages[step_start..step_start + compact_message_count];

    let transcript = render_messages_as_text(compacted_range);
    let summary_prompt = format!(
        "Compress the following execution log of an autonomous coding agent into a high-density, \
         structured milestone summary. Format your summary strictly under these markdown headings:\n\
         ### Milestone Summary (Steps 1-{compact_pairs})\n\
         - **Core Intent & Scope**: The user's original objective and requirements.\n\
         - **Files Modified & Validated**: Exact file paths modified, created, or tested with outcomes.\n\
         - **Key Discoveries & Decisions**: Architectural decisions made, bugs identified, approaches tried and dismissed.\n\
         - **Pending / Next Steps**: Tasks remaining to be completed.\n\n\
         Omit raw stdout/stderr logs and redundant command boilerplate. Focus strictly on facts and state.\n\n{transcript}"
    );

    let summary_result = send_chat_with_fallback(
        config,
        &ChatRequest {
            message: summary_prompt,
            system_instruction: "You compress agent work logs into dense, structured milestone summaries retaining all critical paths, decisions, and pending tasks."
                .into(),
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
            temperature: config.temperature,
            ..Default::default()
        },
    )
    .await;

    match summary_result {
        Ok((summary_response, fallback_provider)) => {
            let mut compacted = Vec::with_capacity(messages.len() - compact_message_count + 3);
            if messages.len() > history_prefix_len {
                compacted.extend_from_slice(&messages[..history_prefix_len]);
                compacted.push(messages[history_prefix_len].clone());
            } else {
                compacted.extend_from_slice(messages);
            }
            compacted.push(ChatMessage {
                role: ChatRole::Assistant,
                content: vec![ContentBlock::Text {
                    text: format!(
                        "[Summary of earlier steps 1-{compact_pairs}, compacted to save context]\n{}",
                        summary_response.text.trim()
                    ),
                }],
            });
            compacted.push(ChatMessage {
                role: ChatRole::User,
                content: vec![ContentBlock::Text {
                    text: "Continue the task using the summary above and the recent steps below."
                        .into(),
                }],
            });
            compacted.extend_from_slice(&messages[step_start + compact_message_count..]);
            Ok(Some((compacted, fallback_provider)))
        }
        Err(err) => {
            // LLM summarization failed (timeout, rate limit 429, provider offline, or context limit).
            // Rather than returning Err and leaving native_messages uncompacted (which causes the next
            // agent turn to crash with context window exceeded), fall back to deterministic hard-truncation.
            let truncated = hard_truncate_native_messages(
                messages,
                history_prefix_len,
                compact_pairs,
                &err.to_string(),
            );
            Ok(Some((
                truncated,
                Some(format!("hard-truncate fallback ({err})")),
            )))
        }
    }
}

pub(super) fn hard_truncate_native_messages(
    messages: &[ChatMessage],
    history_prefix_len: usize,
    compact_pairs: usize,
    error_reason: &str,
) -> Vec<ChatMessage> {
    let step_start = history_prefix_len + 1;
    let compact_message_count = compact_pairs * 2;
    let compacted_range = if messages.len() > step_start {
        let end = (step_start + compact_message_count).min(messages.len());
        &messages[step_start..end]
    } else {
        &[]
    };

    let mut actions = Vec::new();
    for chunk in compacted_range.chunks(2) {
        if let Some(assistant_msg) = chunk.get(0) {
            for block in &assistant_msg.content {
                if let ContentBlock::ToolUse { name, input, .. } = block {
                    let input_summary = match input {
                        serde_json::Value::Object(map) => {
                            if let Some(path) = map.get("path").and_then(|v| v.as_str()) {
                                format!("path: {path}")
                            } else if let Some(cmd) = map.get("command").and_then(|v| v.as_str()) {
                                format!("command: {cmd}")
                            } else if let Some(query) = map.get("query").and_then(|v| v.as_str()) {
                                format!("query: {query}")
                            } else {
                                let s = input.to_string();
                                truncate_for_context(&s, 60)
                            }
                        }
                        _ => {
                            let s = input.to_string();
                            truncate_for_context(&s, 60)
                        }
                    };
                    actions.push(format!("- {name} ({input_summary})"));
                }
            }
        }
    }

    let actions_text = if actions.is_empty() {
        "(no explicit tool calls recorded)".to_string()
    } else {
        actions.join("\n")
    };

    let summary_content = format!(
        "[Summary of steps 1-{compact_pairs} — compacted via deterministic hard-truncation fallback due to summarization failure: {error_reason}]\nActions performed in earlier steps:\n{actions_text}\n[Detailed tool outputs omitted to preserve context window. Recent steps continue below.]"
    );

    let mut compacted =
        Vec::with_capacity(messages.len().saturating_sub(compact_message_count) + 3);
    if messages.len() > history_prefix_len {
        compacted.extend_from_slice(&messages[..history_prefix_len]);
        compacted.push(messages[history_prefix_len].clone());
    } else {
        compacted.extend_from_slice(messages);
    }
    compacted.push(ChatMessage {
        role: ChatRole::Assistant,
        content: vec![ContentBlock::Text {
            text: summary_content,
        }],
    });
    compacted.push(ChatMessage {
        role: ChatRole::User,
        content: vec![ContentBlock::Text {
            text: "Continue the task using the summary above and the recent steps below.".into(),
        }],
    });
    if messages.len() > step_start + compact_message_count {
        compacted.extend_from_slice(&messages[step_start + compact_message_count..]);
    }
    compacted
}

pub(super) fn initial_observation(task: &str, root: &Path, skills: &str) -> String {
    let now = chrono::Local::now()
        .format("%Y-%m-%d %H:%M:%S %Z")
        .to_string();
    let mut observation = format!(
        "Current Time: {now}\nTask: {task}\nWorkspace: {}\nLearned skills:\n{}\n",
        root.display(),
        if skills.trim().is_empty() {
            "(none)"
        } else {
            skills
        }
    );
    if let Ok(memory) = MemoryStore::open_default() {
        if let Ok(Some(name)) = memory.get_profile("name") {
            observation.push_str(&format!("User Name: {name}\n"));
        }
        if let Ok(Some(session)) = memory.workspace_session(&root.to_string_lossy()) {
            observation.push_str(&format!(
                "Previous workspace session ({}):\nSummary: {}\nVerification: {}\n",
                session.updated_at,
                session.summary,
                if session.verification.trim().is_empty() {
                    "(none)"
                } else {
                    &session.verification
                }
            ));
        }
    }
    observation.push_str(&workspace_context(root));
    observation.push_str("Choose the first action. Finish immediately for casual conversation.");
    observation
}

/// Compacts an older trajectory step's verbose observation into a single line preview
/// to prevent `observation` buffer bloat across long-running tasks.
fn compact_trajectory_step_preview(step_str: &str) -> String {
    if let Some(obs_idx) = step_str.find("\n- Observation:") {
        let prefix = &step_str[..obs_idx + "\n- Observation:".len()];
        let obs_text = step_str[obs_idx + "\n- Observation:".len()..].trim();
        if obs_text.chars().count() > 120 {
            let first_line = obs_text
                .lines()
                .find(|l| !l.trim().is_empty())
                .unwrap_or("");
            let truncated = if first_line.chars().count() > 80 {
                format!("{}...", &first_line[..77])
            } else {
                first_line.to_string()
            };
            return format!("{prefix} {truncated} (details collapsed for earlier step)");
        }
    }
    step_str.to_string()
}

/// Rebuilds the `observation` string buffer from the current trajectory in-place,
/// reusing the underlying allocation across agent-loop steps to avoid repeated
/// heap re-allocations and redundant copies.
/// Preserves full detail for the most recent 5 steps while compacting older observations.
pub(super) fn rebuild_observation(
    task: &str,
    root: &Path,
    trajectory: &[String],
    target: &mut String,
) {
    target.clear();
    let trajectory_len: usize = trajectory.iter().map(|s| s.len() + 2).sum();
    let root_display = root.display();
    let estimated = task.len() + 250 + trajectory_len;
    target.reserve(estimated.saturating_sub(target.capacity()));

    use std::fmt::Write as _;
    let _ = write!(
        target,
        "Task: {task}\nWorkspace: {}\n\nHere is the history of what you have done so far in this agent loop:\n\n",
        root_display
    );
    let recent_cutoff = trajectory.len().saturating_sub(5);
    for (i, step) in trajectory.iter().enumerate() {
        if i > 0 {
            target.push_str("\n\n");
        }
        if i < recent_cutoff {
            let compacted = compact_trajectory_step_preview(step);
            target.push_str(&compacted);
        } else {
            target.push_str(step);
        }
    }
    target.push_str(
        "\n\nProceed to the next step. If you have completed the task, use the 'finish' action.",
    );
}

/// Formats a single trajectory step entry with a pre-allocated capacity buffer,
/// avoiding re-allocations when recording actions into the trajectory.
pub(super) fn format_trajectory_step(
    step: usize,
    thought: &str,
    action: &str,
    observation: &str,
) -> String {
    let thought = thought.trim();
    let cap = 35 + thought.len() + action.len() + observation.len();
    let mut s = String::with_capacity(cap);
    use std::fmt::Write as _;
    let _ = write!(
        s,
        "Step {step}:\n- Thought: {thought}\n- Action: {action}\n- Observation: {observation}"
    );
    s
}

/// Keeps full `ContentBlock::Image` payloads only for the most recent
/// `max_retained_steps` tool turns. Older tool turns have their `ContentBlock::Image`
/// removed and their tool result text updated to indicate historical pixels
/// were pruned to reduce prompt cache and token bloat.
///
/// Returns the number of `ContentBlock::Image` entries pruned.
pub(super) fn prune_historic_images_from_native_messages(
    messages: &mut [ChatMessage],
    max_retained_steps: usize,
) -> usize {
    if messages.is_empty() {
        return 0;
    }
    let mut tool_count = 0;
    let mut pruned_count = 0;

    for message in messages.iter_mut().rev() {
        if message.role == ChatRole::Tool {
            tool_count += 1;
            if tool_count > max_retained_steps {
                let had_image = message
                    .content
                    .iter()
                    .any(|block| matches!(block, ContentBlock::Image { .. }));
                if had_image {
                    message.content.retain(|block| {
                        if matches!(block, ContentBlock::Image { .. }) {
                            pruned_count += 1;
                            false
                        } else {
                            true
                        }
                    });
                    for block in &mut message.content {
                        if let ContentBlock::ToolResult { content, .. } = block {
                            if content.contains("— see attached image") {
                                *content = content.replace(
                                    "— see attached image",
                                    "— historical image pixels pruned from context",
                                );
                            } else if !content.contains("pruned from context") {
                                content.push_str(" [historical image pixels pruned from context]");
                            }
                        }
                    }
                }
            }
        }
    }
    pruned_count
}

/// Progressively ages older `ContentBlock::ToolResult` entries out of prompt memory
/// using a Three-Tier Retention Policy (Cursor / Claude Code pattern):
/// - Hot Zone (most recent `hot_steps` tool turns): Preserves full output verbatim.
/// - Warm Zone (turns between `hot_steps + 1` and `hot_steps + warm_steps`):
///   Truncates verbose outputs (> 500 chars) to initial lines + omission note.
/// - Cold Zone (older than `hot_steps + warm_steps`):
///   Collapses output into a 1-line tombstone summary.
///
/// Returns the number of tool results compacted.
pub(super) fn prune_historic_tool_results_from_native_messages(
    messages: &mut [ChatMessage],
    hot_steps: usize,
    warm_steps: usize,
) -> usize {
    if messages.is_empty() {
        return 0;
    }
    let mut tool_count = 0;
    let mut compacted_count = 0;

    for message in messages.iter_mut().rev() {
        if message.role == ChatRole::Tool {
            tool_count += 1;
            if tool_count <= hot_steps {
                // Hot zone: keep full detail
                continue;
            } else if tool_count <= hot_steps + warm_steps {
                // Warm zone: compress if long
                for block in &mut message.content {
                    if let ContentBlock::ToolResult { content, .. } = block {
                        if content.chars().count() > 500
                            && !content.starts_with("[Tool Result:")
                            && !content.starts_with("[read_file")
                        {
                            let lines: Vec<&str> = content.lines().collect();
                            if lines.len() > 12 {
                                let head = lines[..8].join("\n");
                                let omitted = lines.len() - 10;
                                let tail = lines[lines.len() - 2..].join("\n");
                                *content = format!(
                                    "{head}\n\n[... {omitted} lines collapsed for earlier turn ...]\n{tail}"
                                );
                                compacted_count += 1;
                            }
                        }
                    }
                }
            } else {
                // Cold zone: collapse to 1-line tombstone
                for block in &mut message.content {
                    if let ContentBlock::ToolResult {
                        content, is_error, ..
                    } = block
                    {
                        if !content.starts_with("[Tool Result:")
                            && !content.starts_with("[read_file")
                        {
                            if *is_error {
                                let err_line = content
                                    .lines()
                                    .find(|l| !l.trim().is_empty())
                                    .unwrap_or("Failed");
                                let trimmed_err = if err_line.len() > 70 {
                                    format!("{}...", &err_line[..67])
                                } else {
                                    err_line.to_string()
                                };
                                *content = format!(
                                    "[Tool Result: Failed ({trimmed_err}) — log collapsed for past step]"
                                );
                            } else {
                                *content =
                                    "[Tool Result: Succeeded — output collapsed for past step]"
                                        .to_string();
                            }
                            compacted_count += 1;
                        }
                    }
                }
            }
        }
    }
    compacted_count
}

/// When a file edit succeeds in the agent loop, scans earlier `ChatMessage` history
/// for prior `read_file` tool results targeting any of `modified_paths`.
/// Collapses their bodies into a concise note indicating their content is superseded.
/// Prevents the model from reading obsolete code states and saves tokens.
///
/// Returns the number of read_file results superseded.
pub(super) fn supersede_stale_file_reads(
    messages: &mut [ChatMessage],
    modified_paths: &[String],
    current_step: usize,
) -> usize {
    if messages.is_empty() || modified_paths.is_empty() {
        return 0;
    }

    // Map tool_use_id -> file path for any read_file ToolUse targeting modified_paths
    let mut superseded_call_ids: std::collections::HashMap<String, String> =
        std::collections::HashMap::new();

    for message in messages.iter() {
        if message.role == ChatRole::Assistant {
            for block in &message.content {
                if let ContentBlock::ToolUse {
                    id, name, input, ..
                } = block
                {
                    if name == "read_file" {
                        if let Some(target_path) = input.get("path").and_then(|p| p.as_str()) {
                            if modified_paths.iter().any(|m| {
                                m == target_path
                                    || target_path.ends_with(m.as_str())
                                    || m.ends_with(target_path)
                            }) {
                                superseded_call_ids.insert(id.clone(), target_path.to_string());
                            }
                        }
                    }
                }
            }
        }
    }

    if superseded_call_ids.is_empty() {
        return 0;
    }

    let mut superseded_count = 0;
    for message in messages.iter_mut() {
        if message.role == ChatRole::Tool {
            for block in &mut message.content {
                if let ContentBlock::ToolResult {
                    tool_use_id,
                    content,
                    ..
                } = block
                {
                    if let Some(path) = superseded_call_ids.get(tool_use_id) {
                        if !content.starts_with("[read_file") {
                            *content = format!(
                                "[read_file \"{path}\": Content superseded by file modifications at Step {current_step}]"
                            );
                            superseded_count += 1;
                        }
                    }
                }
            }
        }
    }

    superseded_count
}

/// Sends a lifecycle around the actual summarizer await; callers retain the
/// original messages on failure. Short histories do not enter this UI state.
pub(super) async fn report_context_compaction<F>(
    eligible: bool,
    total_tokens: u32,
    window: usize,
    work: F,
    progress: &mut impl FnMut(AgentProgress),
) -> Result<Option<(Vec<ChatMessage>, Option<String>)>, ChatError>
where
    F: Future<Output = Result<Option<(Vec<ChatMessage>, Option<String>)>, ChatError>>,
{
    if !eligible {
        return work.await;
    }
    progress(AgentProgress::ContextCompaction {
        subagent: None,
        status: "started".into(),
        message: "Summarizing earlier steps to continue working".into(),
    });
    let result = work.await;
    let (status, message) = match &result {
        Ok(Some((_, fallback))) => {
            let fallback_note = fallback
                .as_ref()
                .map(|p| {
                    if p.starts_with("hard-truncate") {
                        format!("; {p}")
                    } else {
                        format!("; summary generated via fallback provider {p}")
                    }
                })
                .unwrap_or_default();
            (
                "completed",
                format!(
                    "Context compacted — earlier steps summarized to continue working ({total_tokens}/{window} tokens before compaction){fallback_note}."
                ),
            )
        }
        Ok(None) => (
            "completed",
            "Context unchanged — no earlier steps needed summarizing.".into(),
        ),
        Err(error) => (
            "failed",
            format!(
                "Context compaction failed: {error}. Continuing with the original context; it may still reach the model's limit."
            ),
        ),
    };
    progress(AgentProgress::ContextCompaction {
        subagent: None,
        status: status.into(),
        message,
    });
    result
}

#[cfg(test)]
mod compaction_progress_tests {
    use super::*;
    use std::cell::RefCell;

    #[tokio::test]
    async fn compaction_reports_start_before_await_and_completion_after_success() {
        let events = RefCell::new(Vec::new());
        let work = async {
            assert!(
                matches!(&events.borrow()[0], AgentProgress::ContextCompaction { status, .. } if status == "started")
            );
            Ok(Some((Vec::new(), None)))
        };
        report_context_compaction(true, 75, 100, work, &mut |e| events.borrow_mut().push(e))
            .await
            .unwrap();
        assert!(
            matches!(&events.borrow()[1], AgentProgress::ContextCompaction { status, message, .. } if status == "completed" && message.starts_with("Context compacted"))
        );
    }

    #[tokio::test]
    async fn short_history_never_announces_compaction() {
        let events = RefCell::new(Vec::new());
        report_context_compaction(false, 75, 100, async { Ok(None) }, &mut |e| {
            events.borrow_mut().push(e)
        })
        .await
        .unwrap();
        assert!(events.borrow().is_empty());
    }

    #[tokio::test]
    async fn failed_compaction_reports_original_context_will_be_retained() {
        let events = RefCell::new(Vec::new());
        let result = report_context_compaction(
            true,
            75,
            100,
            async { Err(ChatError::UnsupportedProvider("offline".into())) },
            &mut |e| events.borrow_mut().push(e),
        )
        .await;
        assert!(result.is_err());
        assert!(
            matches!(&events.borrow()[1], AgentProgress::ContextCompaction { status, message, .. } if status == "failed" && message.contains("offline") && message.contains("original context"))
        );
    }

    #[test]
    fn test_prune_historic_images_empty_messages() {
        let mut messages = Vec::new();
        assert_eq!(
            prune_historic_images_from_native_messages(&mut messages, 2),
            0
        );
    }

    #[test]
    fn test_prune_historic_images_keeps_within_retention_window() {
        let mut messages = vec![
            ChatMessage {
                role: ChatRole::User,
                content: vec![ContentBlock::Text {
                    text: "task".into(),
                }],
            },
            ChatMessage {
                role: ChatRole::Assistant,
                content: vec![ContentBlock::Text {
                    text: "call 1".into(),
                }],
            },
            ChatMessage {
                role: ChatRole::Tool,
                content: vec![
                    ContentBlock::ToolResult {
                        tool_use_id: "c1".into(),
                        content: "[Image loaded — see attached image]".into(),
                        is_error: false,
                    },
                    ContentBlock::Image {
                        data_uri: "data:image/png;base64,img1".into(),
                    },
                ],
            },
            ChatMessage {
                role: ChatRole::Assistant,
                content: vec![ContentBlock::Text {
                    text: "call 2".into(),
                }],
            },
            ChatMessage {
                role: ChatRole::Tool,
                content: vec![
                    ContentBlock::ToolResult {
                        tool_use_id: "c2".into(),
                        content: "[Image loaded — see attached image]".into(),
                        is_error: false,
                    },
                    ContentBlock::Image {
                        data_uri: "data:image/png;base64,img2".into(),
                    },
                ],
            },
        ];

        // Retention is 2, and there are 2 tool messages, so none should be pruned.
        let pruned = prune_historic_images_from_native_messages(&mut messages, 2);
        assert_eq!(pruned, 0);
        assert_eq!(messages[2].content.len(), 2);
        assert_eq!(messages[4].content.len(), 2);
    }

    #[test]
    fn test_prune_historic_images_prunes_older_than_retention_window() {
        let mut messages = vec![
            ChatMessage {
                role: ChatRole::User,
                content: vec![ContentBlock::Text {
                    text: "task".into(),
                }],
            },
            // Step 1
            ChatMessage {
                role: ChatRole::Assistant,
                content: vec![ContentBlock::Text {
                    text: "step 1".into(),
                }],
            },
            ChatMessage {
                role: ChatRole::Tool,
                content: vec![
                    ContentBlock::ToolResult {
                        tool_use_id: "c1".into(),
                        content: "[Image loaded — see attached image]".into(),
                        is_error: false,
                    },
                    ContentBlock::Image {
                        data_uri: "data:image/png;base64,img1".into(),
                    },
                ],
            },
            // Step 2
            ChatMessage {
                role: ChatRole::Assistant,
                content: vec![ContentBlock::Text {
                    text: "step 2".into(),
                }],
            },
            ChatMessage {
                role: ChatRole::Tool,
                content: vec![
                    ContentBlock::ToolResult {
                        tool_use_id: "c2".into(),
                        content: "[Image loaded — see attached image]".into(),
                        is_error: false,
                    },
                    ContentBlock::Image {
                        data_uri: "data:image/png;base64,img2".into(),
                    },
                ],
            },
            // Step 3
            ChatMessage {
                role: ChatRole::Assistant,
                content: vec![ContentBlock::Text {
                    text: "step 3".into(),
                }],
            },
            ChatMessage {
                role: ChatRole::Tool,
                content: vec![
                    ContentBlock::ToolResult {
                        tool_use_id: "c3".into(),
                        content: "[Image loaded — see attached image]".into(),
                        is_error: false,
                    },
                    ContentBlock::Image {
                        data_uri: "data:image/png;base64,img3".into(),
                    },
                ],
            },
        ];

        // Retention is 2: Step 3 (most recent) and Step 2 are kept; Step 1 is pruned.
        let pruned = prune_historic_images_from_native_messages(&mut messages, 2);
        assert_eq!(pruned, 1);

        // Step 1: Image removed, ToolResult updated
        assert_eq!(messages[2].content.len(), 1);
        match &messages[2].content[0] {
            ContentBlock::ToolResult { content, .. } => {
                assert!(content.contains("historical image pixels pruned from context"));
                assert!(!content.contains("— see attached image"));
            }
            _ => panic!("Expected ToolResult block"),
        }

        // Step 2: Image kept
        assert_eq!(messages[4].content.len(), 2);
        assert!(matches!(messages[4].content[1], ContentBlock::Image { .. }));

        // Step 3: Image kept
        assert_eq!(messages[6].content.len(), 2);
        assert!(matches!(messages[6].content[1], ContentBlock::Image { .. }));
    }

    fn sample_step_pair(index: usize) -> [ChatMessage; 2] {
        [
            ChatMessage {
                role: ChatRole::Assistant,
                content: vec![ContentBlock::ToolUse {
                    id: format!("call{index}"),
                    name: "read_file".into(),
                    input: serde_json::json!({"path": format!("file{index}.rs")}),
                    thought_signature: None,
                }],
            },
            ChatMessage {
                role: ChatRole::Tool,
                content: vec![ContentBlock::ToolResult {
                    tool_use_id: format!("call{index}"),
                    content: format!("contents of file{index}.rs"),
                    is_error: false,
                }],
            },
        ]
    }

    #[test]
    fn render_messages_as_text_truncates_oversized_tool_results() {
        let long_content = "x".repeat(10_000);
        let messages = vec![
            ChatMessage {
                role: ChatRole::User,
                content: vec![ContentBlock::Text {
                    text: "task".into(),
                }],
            },
            ChatMessage {
                role: ChatRole::Assistant,
                content: vec![ContentBlock::ToolUse {
                    id: "c1".into(),
                    name: "cat_huge_file".into(),
                    input: serde_json::json!({}),
                    thought_signature: None,
                }],
            },
            ChatMessage {
                role: ChatRole::Tool,
                content: vec![ContentBlock::ToolResult {
                    tool_use_id: "c1".into(),
                    content: long_content,
                    is_error: false,
                }],
            },
        ];
        let rendered = render_messages_as_text(&messages);
        assert!(rendered.contains("Truncated"));
        assert!(rendered.len() < 3000);
    }

    #[test]
    fn hard_truncate_native_messages_preserves_task_and_role_alternation() {
        let mut messages = vec![ChatMessage::text(ChatRole::User, "initial task")];
        for i in 0..5 {
            messages.extend(sample_step_pair(i));
        }
        // 5 step pairs total (11 messages). Compact 2 pairs (steps 0 and 1), keeping steps 2, 3, 4 (3 pairs = 6 msgs).
        let truncated = hard_truncate_native_messages(&messages, 0, 2, "mock provider failure");

        assert_eq!(truncated.len(), 1 + 2 + 6);
        assert_eq!(truncated[0].role, ChatRole::User);
        assert_eq!(truncated[1].role, ChatRole::Assistant);
        assert_eq!(truncated[2].role, ChatRole::User);

        // Verify summary contains key info
        match &truncated[1].content[0] {
            ContentBlock::Text { text } => {
                assert!(text.contains("mock provider failure"));
                assert!(text.contains("read_file"));
                assert!(text.contains("file0.rs"));
                assert!(text.contains("file1.rs"));
            }
            _ => panic!("Expected Text block in summary"),
        }

        // Verify role sequence after compaction:
        // [0] User (initial task)
        // [1] Assistant (compacted summary)
        // [2] User (continuation prompt)
        // [3..] Alternating Assistant (ToolUse) and Tool (ToolResult)
        for (i, msg) in truncated.iter().enumerate().skip(3) {
            let expected_role = if i % 2 == 1 {
                ChatRole::Assistant
            } else {
                ChatRole::Tool
            };
            assert_eq!(msg.role, expected_role);
        }
    }

    #[tokio::test]
    async fn compact_native_messages_falls_back_to_hard_truncate_on_error() {
        let mut messages = vec![ChatMessage::text(ChatRole::User, "initial task")];
        // 5 step pairs = 2 to compact (since KEEP_RECENT = 3)
        for i in 0..5 {
            messages.extend(sample_step_pair(i));
        }
        let config = MintConfig {
            ai_provider: "non_existent_provider_that_will_fail".into(),
            ..Default::default()
        };
        let result = compact_native_messages(&config, &messages, 0)
            .await
            .unwrap();
        assert!(result.is_some());
        let (compacted, fallback) = result.unwrap();
        assert_eq!(compacted.len(), 1 + 2 + 6);
        assert_eq!(compacted[1].role, ChatRole::Assistant);
        assert_eq!(compacted[2].role, ChatRole::User);
        assert!(fallback.as_deref().unwrap_or("").contains("hard-truncate"));
    }

    #[tokio::test]
    async fn compact_native_messages_preserves_conversation_history_prefix() {
        // Build 2 historical conversation turns (4 messages: User, Assistant, User, Assistant)
        let mut messages = vec![
            ChatMessage::text(ChatRole::User, "ประวัติคุยรอบ 1"),
            ChatMessage::text(ChatRole::Assistant, "ตอบประวัติรอบ 1"),
            ChatMessage::text(ChatRole::User, "ประวัติคุยรอบ 2"),
            ChatMessage::text(ChatRole::Assistant, "ตอบประวัติรอบ 2"),
        ];
        let history_prefix_len = messages.len();

        // Current agent turn initial observation
        messages.push(ChatMessage::text(ChatRole::User, "งานปัจจุบันรอบ 3"));

        // Current turn execution steps: 5 step pairs (10 messages)
        for i in 0..5 {
            messages.extend(sample_step_pair(i));
        }

        let config = MintConfig {
            ai_provider: "non_existent_provider_that_will_fail".into(),
            ..Default::default()
        };

        let result = compact_native_messages(&config, &messages, history_prefix_len)
            .await
            .unwrap();
        assert!(result.is_some());
        let (compacted, _) = result.unwrap();

        // Expected structure:
        // [0..4] Historical conversation messages intact
        // [4] Current turn initial task intact
        // [5] Assistant compacted summary of steps 1-2
        // [6] User continuation prompt
        // [7..13] Recent 3 step pairs (6 messages)
        assert_eq!(compacted.len(), 4 + 1 + 2 + 6);

        // Verify history messages remain identical
        assert_eq!(compacted[0].role, ChatRole::User);
        assert_eq!(
            compacted[0].content,
            vec![ContentBlock::Text {
                text: "ประวัติคุยรอบ 1".into()
            }]
        );
        assert_eq!(compacted[1].role, ChatRole::Assistant);
        assert_eq!(
            compacted[1].content,
            vec![ContentBlock::Text {
                text: "ตอบประวัติรอบ 1".into()
            }]
        );
        assert_eq!(compacted[2].role, ChatRole::User);
        assert_eq!(
            compacted[2].content,
            vec![ContentBlock::Text {
                text: "ประวัติคุยรอบ 2".into()
            }]
        );
        assert_eq!(compacted[3].role, ChatRole::Assistant);
        assert_eq!(
            compacted[3].content,
            vec![ContentBlock::Text {
                text: "ตอบประวัติรอบ 2".into()
            }]
        );

        // Verify current turn initial task intact
        assert_eq!(compacted[4].role, ChatRole::User);
        assert_eq!(
            compacted[4].content,
            vec![ContentBlock::Text {
                text: "งานปัจจุบันรอบ 3".into()
            }]
        );

        // Verify summary block
        assert_eq!(compacted[5].role, ChatRole::Assistant);
        match &compacted[5].content[0] {
            ContentBlock::Text { text } => {
                assert!(text.contains("Summary of steps 1-2"));
                assert!(text.contains("file0.rs"));
                assert!(text.contains("file1.rs"));
            }
            _ => panic!("Expected Text block in summary"),
        }

        // Verify continuation prompt
        assert_eq!(compacted[6].role, ChatRole::User);

        // Verify recent step pairs preserved
        assert_eq!(compacted[7].role, ChatRole::Assistant);
        assert_eq!(compacted[8].role, ChatRole::Tool);
    }

    #[test]
    fn test_prune_historic_tool_results_progressive_aging() {
        let mut messages = vec![ChatMessage::text(ChatRole::User, "initial task")];
        // Create 15 steps with verbose outputs (> 500 chars)
        for i in 0..15 {
            let verbose_log =
                format!("Line 1 of step {i}: compiler diagnostic message\n").repeat(30);
            messages.push(ChatMessage {
                role: ChatRole::Assistant,
                content: vec![ContentBlock::ToolUse {
                    id: format!("call{i}"),
                    name: "run_shell".into(),
                    input: serde_json::json!({"cmd": "cargo check"}),
                    thought_signature: None,
                }],
            });
            messages.push(ChatMessage {
                role: ChatRole::Tool,
                content: vec![ContentBlock::ToolResult {
                    tool_use_id: format!("call{i}"),
                    content: verbose_log,
                    is_error: false,
                }],
            });
        }

        // 3 hot steps, 7 warm steps
        let compacted = prune_historic_tool_results_from_native_messages(&mut messages, 3, 7);
        assert!(compacted > 0);

        // Tool messages are at indices 2, 4, 6, ..., 30.
        // Step 14 is index 30 (hot, tool_count = 1) -> full text
        // Step 13 is index 28 (hot, tool_count = 2) -> full text
        // Step 12 is index 26 (hot, tool_count = 3) -> full text
        let msg_hot = &messages[30];
        if let ContentBlock::ToolResult { content, .. } = &msg_hot.content[0] {
            assert!(content.contains("Line 1 of step 14"));
            assert!(!content.contains("collapsed"));
        }

        // Step 11 is index 24 (warm, tool_count = 4) -> warm collapsed
        let msg_warm = &messages[24];
        if let ContentBlock::ToolResult { content, .. } = &msg_warm.content[0] {
            assert!(content.contains("lines collapsed for earlier turn"));
        }

        // Step 0 is index 2 (cold, tool_count = 15) -> tombstone
        let msg_cold = &messages[2];
        if let ContentBlock::ToolResult { content, .. } = &msg_cold.content[0] {
            assert_eq!(
                content,
                "[Tool Result: Succeeded — output collapsed for past step]"
            );
        }
    }

    #[test]
    fn test_supersede_stale_file_reads() {
        let mut messages = vec![ChatMessage::text(ChatRole::User, "initial task")];
        // Step 1: read_file "src/main.rs"
        messages.push(ChatMessage {
            role: ChatRole::Assistant,
            content: vec![ContentBlock::ToolUse {
                id: "call_read_1".into(),
                name: "read_file".into(),
                input: serde_json::json!({"path": "src/main.rs"}),
                thought_signature: None,
            }],
        });
        messages.push(ChatMessage {
            role: ChatRole::Tool,
            content: vec![ContentBlock::ToolResult {
                tool_use_id: "call_read_1".into(),
                content: "fn main() { println!(\"old code\"); }".into(),
                is_error: false,
            }],
        });

        // Step 2: edit src/main.rs
        let superseded = supersede_stale_file_reads(&mut messages, &["src/main.rs".to_string()], 2);
        assert_eq!(superseded, 1);

        if let ContentBlock::ToolResult { content, .. } = &messages[2].content[0] {
            assert!(content.contains("Content superseded by file modifications at Step 2"));
            assert!(!content.contains("old code"));
        }
    }

    #[test]
    fn test_rebuild_observation_compacts_older_trajectory_steps() {
        let root = Path::new("/workspace");
        let task = "Build project";
        let mut trajectory = Vec::new();
        for i in 1..=8 {
            let obs = format!("Long compiler output line for step {i}\n").repeat(10);
            trajectory.push(format_trajectory_step(
                i,
                "Checking code",
                "run_shell",
                &obs,
            ));
        }

        let mut buffer = String::new();
        rebuild_observation(task, root, &trajectory, &mut buffer);

        // Step 1 is older than 5 steps back -> compacted
        assert!(buffer.contains("details collapsed for earlier step"));
        // Step 8 is recent -> full observation intact
        assert!(buffer.contains("Long compiler output line for step 8"));
    }
}

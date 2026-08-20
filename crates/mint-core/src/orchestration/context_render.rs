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
                    ContentBlock::ToolResult { content, .. } => format!("Result: {content}"),
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
/// — not a failure, just a no-op. `Err` means compaction was attempted but the
/// summarization call itself failed; compaction is a best-effort optimization,
/// so callers should fall back to the uncompacted messages rather than failing
/// the agent run, but may want to surface the failure differently than a
/// routine no-op.
pub(super) async fn compact_native_messages(
    config: &MintConfig,
    messages: &[ChatMessage],
) -> Result<Option<Vec<ChatMessage>>, ChatError> {
    let step_pairs = messages.len().saturating_sub(1) / 2;
    if step_pairs <= COMPACTION_KEEP_RECENT_STEPS || messages.is_empty() {
        return Ok(None);
    }
    let compact_pairs = step_pairs - COMPACTION_KEEP_RECENT_STEPS;
    let compact_message_count = compact_pairs * 2;
    let compacted_range = &messages[1..1 + compact_message_count];

    let transcript = render_messages_as_text(compacted_range);
    let summary_prompt = format!(
        "Summarize the following part of an autonomous coding agent's work log concisely but \
         completely. Preserve: exact file paths touched and their resulting state, exact \
         commands run and whether they succeeded, key findings from searches/reads, and any \
         decisions or open threads still relevant to finishing the task. Omit verbose \
         stdout/stderr detail that isn't load-bearing. Write it as dense prose, not a copy of \
         the log.\n\n{transcript}"
    );

    let (summary_response, _) = send_chat_with_fallback(
        config,
        &ChatRequest {
            message: summary_prompt,
            system_instruction: "You compress agent work logs into dense, factual summaries."
                .into(),
            chat_id: None,
            image_data_uri: None,
            audio_data_uri: None,
            video_data_uri: None,
            document_attachment: None,
            workspace_path: None,
            agent_id: None,
            plan_mode: false,
            messages: None,
            tools: None,
        },
    )
    .await?;

    let mut compacted = Vec::with_capacity(messages.len() - compact_message_count + 3);
    compacted.push(messages[0].clone());
    compacted.push(ChatMessage {
        role: ChatRole::Assistant,
        content: vec![ContentBlock::ToolUse {
            id: "compacted_summary".into(),
            name: "conversation_summary".into(),
            input: serde_json::json!({}),
            thought_signature: None,
        }],
    });
    compacted.push(ChatMessage {
        role: ChatRole::Tool,
        content: vec![ContentBlock::ToolResult {
            tool_use_id: "compacted_summary".into(),
            content: format!(
                "[Summary of steps 1-{compact_pairs}, compacted to save context]\n{}",
                summary_response.text.trim()
            ),
            is_error: false,
        }],
    });
    compacted.extend_from_slice(&messages[1 + compact_message_count..]);
    Ok(Some(compacted))
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

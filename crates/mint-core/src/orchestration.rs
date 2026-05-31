use thiserror::Error;

use crate::{
    ChatError, ChatRequest, ChatResponse, MemoryError, MemoryStore, MintConfig, send_chat,
};

const CONTEXT_LIMIT: usize = 6;

#[derive(Debug, Error)]
pub enum OrchestrationError {
    #[error(transparent)]
    Chat(#[from] ChatError),
    #[error(transparent)]
    Memory(#[from] MemoryError),
}

pub async fn orchestrate_chat(
    config: &MintConfig,
    request: &ChatRequest,
) -> Result<ChatResponse, OrchestrationError> {
    let memory = MemoryStore::open_default()?;
    let mut interactions = memory.recent_interactions(CONTEXT_LIMIT)?;
    interactions.reverse();

    let mut enriched = request.clone();
    let transcript = interactions
        .into_iter()
        .map(|item| format!("User: {}\nAssistant: {}", item.user_text, item.ai_text))
        .collect::<Vec<_>>()
        .join("\n\n");
    if !transcript.is_empty() {
        enriched.system_instruction = format!(
            "{}\n\nRecent conversation context:\n{}",
            enriched.system_instruction.trim(),
            transcript
        )
        .trim()
        .to_owned();
    }

    let response = send_chat(config, &enriched).await?;
    memory.add_interaction(&request.message, &response.text)?;
    Ok(response)
}

pub fn stream_chunks(text: &str) -> Vec<String> {
    let mut chunks = Vec::new();
    let mut current = String::new();
    for word in text.split_whitespace() {
        if !current.is_empty() {
            current.push(' ');
        }
        current.push_str(word);
        if current.len() >= 36 || word.ends_with(['.', '!', '?', '\n']) {
            chunks.push(std::mem::take(&mut current));
        }
    }
    if !current.is_empty() {
        chunks.push(current);
    }
    chunks
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splits_stream_text_without_losing_words() {
        let chunks = stream_chunks("Mint streams short responses back to the desktop renderer.");
        assert_eq!(
            chunks.join(" "),
            "Mint streams short responses back to the desktop renderer."
        );
        assert!(chunks.len() > 1);
    }
}

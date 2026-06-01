use thiserror::Error;

use crate::chat::{send_chat_with_fallback, stream_chat_with_fallback};
use crate::{
    ChatError, ChatRequest, ChatResponse, MemoryError, MemoryStore, MintConfig, send_chat,
    stream_chat,
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
    let enriched = enrich_request(&memory, request)?;
    let response = send_chat(config, &enriched).await?;
    memory.add_interaction_with_metadata(
        &request.message,
        &response.text,
        &response.provider,
        &response.model,
    )?;
    Ok(response)
}

pub async fn orchestrate_chat_stream<F>(
    config: &MintConfig,
    request: &ChatRequest,
    on_chunk: F,
) -> Result<ChatResponse, OrchestrationError>
where
    F: FnMut(String),
{
    let memory = MemoryStore::open_default()?;
    let enriched = enrich_request(&memory, request)?;
    let response = stream_chat(config, &enriched, on_chunk).await?;
    memory.add_interaction_with_metadata(
        &request.message,
        &response.text,
        &response.provider,
        &response.model,
    )?;
    Ok(response)
}

pub async fn orchestrate_chat_with_fallback(
    config: &MintConfig,
    request: &ChatRequest,
) -> Result<(ChatResponse, Option<String>), OrchestrationError> {
    let memory = MemoryStore::open_default()?;
    let enriched = enrich_request(&memory, request)?;
    let (response, fallback) = send_chat_with_fallback(config, &enriched).await?;
    memory.add_interaction_with_metadata(
        &request.message,
        &response.text,
        &response.provider,
        &response.model,
    )?;
    Ok((response, fallback))
}

pub async fn orchestrate_chat_stream_with_fallback<F>(
    config: &MintConfig,
    request: &ChatRequest,
    on_chunk: F,
) -> Result<(ChatResponse, Option<String>), OrchestrationError>
where
    F: FnMut(String),
{
    let memory = MemoryStore::open_default()?;
    let enriched = enrich_request(&memory, request)?;
    let (response, fallback) = stream_chat_with_fallback(config, &enriched, on_chunk).await?;
    memory.add_interaction_with_metadata(
        &request.message,
        &response.text,
        &response.provider,
        &response.model,
    )?;
    Ok((response, fallback))
}

fn enrich_request(memory: &MemoryStore, request: &ChatRequest) -> Result<ChatRequest, MemoryError> {
    let mut interactions = memory.recent_interactions(CONTEXT_LIMIT)?;
    interactions.reverse();
    let transcript = interactions
        .into_iter()
        .map(|item| format!("User: {}\nAssistant: {}", item.user_text, item.ai_text))
        .collect::<Vec<_>>()
        .join("\n\n");
    let mut enriched = request.clone();
    if !transcript.is_empty() {
        enriched.system_instruction = format!(
            "{}\n\nRecent conversation context:\n{}",
            enriched.system_instruction.trim(),
            transcript
        )
        .trim()
        .to_owned();
    }
    Ok(enriched)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preserves_request_without_history() {
        let store = MemoryStore::open(
            std::env::temp_dir().join(format!("mint-orchestrator-{}.sqlite", std::process::id())),
        );
        let request = ChatRequest {
            message: "hello".into(),
            system_instruction: "system".into(),
            image_data_uri: None,
            audio_data_uri: None,
        };
        assert_eq!(
            enrich_request(&store, &request).unwrap().system_instruction,
            "system"
        );
    }
}

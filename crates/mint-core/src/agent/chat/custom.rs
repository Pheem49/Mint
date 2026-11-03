use reqwest::Client;
use serde_json::Value;

use crate::MintConfig;

use super::{
    ChatError, ChatRequest, ChatStreamEvent, ProviderReply,
    openai::{openai_chat_payload, parse_openai_style_reply, send_openai_style_request},
    provider_key, required_key,
    streaming::{StreamFormat, collect_stream_reply},
};

pub(super) fn resolve_custom_model(config: &MintConfig) -> String {
    let provider_id = config
        .ai_provider
        .strip_prefix("custom:")
        .unwrap_or(&config.ai_provider);

    // Check if a per-provider model selection was persisted in extra config.
    if let Some(selections) = config
        .extra
        .get("customModelSelections")
        .and_then(|v| v.as_object())
        && let Some(model) = selections.get(provider_id).and_then(|v| v.as_str())
        && !model.is_empty()
    {
        return model.to_owned();
    }

    // Fall back to the first model in the provider definition.
    config
        .resolve_custom_provider(&config.ai_provider)
        .and_then(|cp| cp.models.first())
        .map(|m| m.model_id.clone())
        .unwrap_or_default()
}

pub(super) async fn call_huggingface(
    client: &Client,
    config: &MintConfig,
    request: &ChatRequest,
) -> Result<ProviderReply, ChatError> {
    let api_key = provider_key(&config.hf_api_key, "HF_TOKEN");
    required_key("huggingface", &api_key)?;
    let model = config.hf_model.clone();
    // Hugging Face's router is an OpenAI Chat Completions-compatible
    // endpoint, so it can (and must) take the same native `messages`/`tools`
    // payload `call_openai` builds — this used to hand-roll a bare
    // `{system, user: request.message}` request instead, which ignored the
    // agent loop's full tool-call history and `tools` catalog. `message` is
    // deliberately left empty (see the step-loop in `orchestration/mod.rs`)
    // whenever `messages` carries the real conversation, so any mid-task
    // fallback onto this provider was silently losing all task context —
    // the model would see an empty user turn and reply with generic
    // small talk, which the agent loop then treated as the final answer.
    let response: Value = send_openai_style_request(
        client
            .post("https://router.huggingface.co/v1/chat/completions")
            .bearer_auth(api_key)
            .json(&openai_chat_payload(config, &model, request, false)?),
        "huggingface",
    )
    .await?;
    parse_openai_style_reply(model, &response)
}

pub(super) async fn stream_huggingface_reply<F>(
    client: &Client,
    config: &MintConfig,
    request: &ChatRequest,
    on_event: &mut F,
) -> Result<ProviderReply, ChatError>
where
    F: FnMut(ChatStreamEvent),
{
    let api_key = provider_key(&config.hf_api_key, "HF_TOKEN");
    required_key("huggingface", &api_key)?;
    let model = config.hf_model.clone();
    let response = client
        .post("https://router.huggingface.co/v1/chat/completions")
        .bearer_auth(api_key)
        .json(&openai_chat_payload(config, &model, request, true)?)
        .send()
        .await?
        .error_for_status()?;
    collect_stream_reply(response, StreamFormat::OpenAi, model, on_event).await
}

pub(super) async fn call_custom_provider(
    client: &Client,
    config: &MintConfig,
    request: &ChatRequest,
) -> Result<ProviderReply, ChatError> {
    let provider = config
        .resolve_custom_provider(&config.ai_provider)
        .ok_or_else(|| ChatError::UnsupportedProvider(config.ai_provider.clone()))?;

    let base_url = provider.base_url.trim_end_matches('/');
    let model = resolve_custom_model(config);
    let payload = openai_chat_payload(config, &model, request, false)?;

    let mut builder = client
        .post(format!("{base_url}/chat/completions"))
        .json(&payload);

    if !provider.api_key.is_empty() {
        builder = builder.bearer_auth(&provider.api_key);
    }
    for h in &provider.headers {
        if !h.name.is_empty() {
            builder = builder.header(h.name.as_str(), h.value.as_str());
        }
    }

    let response: serde_json::Value = builder.send().await?.error_for_status()?.json().await?;

    parse_openai_style_reply(model, &response)
}

pub(super) async fn stream_custom_provider_reply<F>(
    client: &Client,
    config: &MintConfig,
    request: &ChatRequest,
    on_event: &mut F,
) -> Result<ProviderReply, ChatError>
where
    F: FnMut(ChatStreamEvent),
{
    let provider = config
        .resolve_custom_provider(&config.ai_provider)
        .ok_or_else(|| ChatError::UnsupportedProvider(config.ai_provider.clone()))?;
    let model = resolve_custom_model(config);
    let mut builder = client
        .post(format!(
            "{}/chat/completions",
            provider.base_url.trim_end_matches('/')
        ))
        .json(&openai_chat_payload(config, &model, request, true)?);
    if !provider.api_key.is_empty() {
        builder = builder.bearer_auth(&provider.api_key);
    }
    for header in &provider.headers {
        if !header.name.is_empty() {
            builder = builder.header(header.name.as_str(), header.value.as_str());
        }
    }
    let response = builder.send().await?.error_for_status()?;
    collect_stream_reply(response, StreamFormat::OpenAi, model, on_event).await
}

//! Provider → model presets, shared by the slash engine's `/models` picker, the
//! CLI onboarding wizard (`crates/mint-cli/src/onboard.rs`), and the CLI's
//! `model_options_for_provider` (`crates/mint-cli/src/interactive/confirm.rs`).
//!
//! For live model lists prefer [`model_options_for_provider_async`], which tries
//! the provider's API first and falls back to the static presets here.

use crate::MintConfig;

pub const GEMINI_MODEL_PRESETS: &[&str] = &[
    "gemini-3.6-flash",
    "gemini-3.5-flash",
    "gemini-3.5-flash-lite",
    "gemini-3.1-flash-lite",
    "gemini-2.5-flash",
];

pub const ANTHROPIC_MODEL_PRESETS: &[&str] = &[
    "claude-sonnet-5",
    "claude-opus-5",
    "claude-sonnet-4.6",
    "claude-opus-4.8",
    "claude-haiku-4.5",
];

pub const OPENAI_MODEL_PRESETS: &[&str] = &[
    "gpt-5.6-sol",
    "gpt-5.6-terra",
    "gpt-5.6-luna",
    "gpt-5.5-thinking",
    "gpt-5.5-pro",
];

pub const OPENROUTER_MODEL_PRESETS: &[&str] = &[
    "openai/gpt-5.6-terra",
    "anthropic/claude-sonnet-5",
    "google/gemini-3.6-flash",
    "x-ai/grok-4.5",
    "deepseek/deepseek-v4-pro",
];

pub const DEEPSEEK_MODEL_PRESETS: &[&str] = &[
    "deepseek-v4-flash",
    "deepseek-v4-pro",
    "deepseek-chat",
    "deepseek-reasoner",
];

pub const HUGGINGFACE_MODEL_PRESETS: &[&str] = &[
    "Qwen/Qwen3.6-27B",
    "deepseek-ai/DeepSeek-V4-Flash",
    "google/gemma-3-27b-it",
    "meta-llama/Llama-3.3-70B-Instruct",
    "microsoft/phi-4",
    "zai-org/GLM-5.2-FP8",
    "mistralai/Mistral-Large-Instruct",
    "openai/gpt-oss-120b",
];

/// Locally installed Ollama models, from `ollama list`. Empty when Ollama isn't
/// installed or running.
pub fn installed_ollama_models() -> Vec<String> {
    let output = match std::process::Command::new("ollama").arg("list").output() {
        Ok(output) if output.status.success() => output,
        _ => return Vec::new(),
    };
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .skip(1)
        .filter_map(|line| line.split_whitespace().next())
        .filter(|name| !name.trim().is_empty())
        .map(|name| name.to_string())
        .collect()
}

/// Human-facing provider name, matching the frontend's `providerLabel()`
/// (`src/renderer/shared/utils/providers.ts`) so the `/models` confirmation and
/// the `provider_change` chip read the same on every surface.
pub fn provider_display_name(config: &MintConfig, provider: &str) -> String {
    match provider {
        "gemini" => "Gemini".into(),
        "openai" => "OpenAI".into(),
        "openrouter" => "OpenRouter".into(),
        "deepseek" => "DeepSeek".into(),
        "anthropic" => "Claude".into(),
        "huggingface" => "Hugging Face".into(),
        "local_openai" => "Local OpenAI".into(),
        "ollama" => "Ollama".into(),
        p if p.starts_with("custom:") => config
            .resolve_custom_provider(p)
            .map(|cp| cp.display_name.clone())
            .unwrap_or_else(|| p.to_string()),
        other => other.to_string(),
    }
}

/// Suggested model ids for `provider`. Static presets for the hosted providers,
/// a live `ollama list` for `ollama`, and the configured model ids for a
/// `custom:<id>` provider. Empty when nothing is known.
pub fn model_options_for_provider(config: &MintConfig, provider: &str) -> Vec<String> {
    match provider {
        "gemini" => GEMINI_MODEL_PRESETS,
        "anthropic" => ANTHROPIC_MODEL_PRESETS,
        "openai" => OPENAI_MODEL_PRESETS,
        "openrouter" => OPENROUTER_MODEL_PRESETS,
        "deepseek" => DEEPSEEK_MODEL_PRESETS,
        "huggingface" => HUGGINGFACE_MODEL_PRESETS,
        "ollama" => return installed_ollama_models(),
        p if p.starts_with("custom:") => {
            return config
                .resolve_custom_provider(p)
                .map(|cp| cp.models.iter().map(|m| m.model_id.clone()).collect())
                .unwrap_or_default();
        }
        _ => &[],
    }
    .iter()
    .map(|s| s.to_string())
    .collect()
}

/// Like [`model_options_for_provider`] but tries the provider's live API first.
///
/// On success the live list replaces the static presets for this call; on any
/// network/auth error it silently falls back to the static presets, so callers
/// always get *something* useful even when offline or before the user has
/// entered an API key.
///
/// Results are cached for 1 hour inside [`super::model_fetcher`].
pub async fn model_options_for_provider_async(
    config: &MintConfig,
    provider: &str,
) -> Vec<String> {
    use super::model_fetcher;

    // Resolve the API key and optional base URL for this provider.
    let (api_key, base_url): (&str, Option<&str>) = match provider {
        "gemini"       => (&config.api_key, None),
        "anthropic"    => (&config.anthropic_api_key, None),
        "openai"       => (&config.openai_api_key, None),
        "openrouter"   => (&config.openrouter_api_key, None),
        "deepseek"     => (&config.deepseek_api_key, None),
        "local_openai" => ("", Some(config.local_api_base_url.as_str())),
        // Ollama and HuggingFace keep their existing sync paths.
        _ => return model_options_for_provider(config, provider),
    };

    let dynamic = model_fetcher::fetch_provider_models(provider, api_key, base_url).await;
    if !dynamic.is_empty() {
        // Merge: put dynamic list first, then append any presets not already present.
        let mut merged = dynamic;
        for preset in model_options_for_provider(config, provider) {
            if !merged.contains(&preset) {
                merged.push(preset);
            }
        }
        merged
    } else {
        // Network/auth error → fall back to static presets.
        model_options_for_provider(config, provider)
    }
}

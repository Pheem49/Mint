//! Shared image, video and Gemini Live model choices for CLI, Desktop and HTTP.
//!
//! Nonblank credential overrides take precedence over saved provider keys.
//! Discovery results come first, followed by presets not already discovered.
//! Missing credentials or failed discovery therefore still leave preset choices.

use std::future::Future;

use crate::MintConfig;

const GEMINI_LIVE_MODEL_PRESETS: &[&str] = &[
    "gemini-2.5-flash-native-audio-preview-12-2025",
    "gemini-3.1-flash-live-preview",
];

/// Image model choices using the override key, or this provider's saved key.
pub async fn image_models(
    config: &MintConfig,
    provider: &str,
    api_key_override: &str,
) -> Vec<String> {
    models_with_discovery(
        config,
        ModelKind::Image,
        provider,
        api_key_override,
        discover,
    )
    .await
}

/// Video model choices using the override key, or this provider's saved key.
pub async fn video_models(
    config: &MintConfig,
    provider: &str,
    api_key_override: &str,
) -> Vec<String> {
    models_with_discovery(
        config,
        ModelKind::Video,
        provider,
        api_key_override,
        discover,
    )
    .await
}

/// Gemini Live model choices using the override key, or the saved Gemini key.
pub async fn live_models(config: &MintConfig, api_key_override: &str) -> Vec<String> {
    models_with_discovery(
        config,
        ModelKind::Live,
        "gemini",
        api_key_override,
        discover,
    )
    .await
}

#[derive(Clone, Copy)]
enum ModelKind {
    Image,
    Video,
    Live,
}

async fn models_with_discovery<F, Fut>(
    config: &MintConfig,
    kind: ModelKind,
    provider: &str,
    api_key_override: &str,
    discover: F,
) -> Vec<String>
where
    F: FnOnce(ModelKind, String, String) -> Fut,
    Fut: Future<Output = Vec<String>>,
{
    let provider = provider.trim().to_lowercase();
    let (saved_key, presets): (&str, Vec<String>) = match kind {
        ModelKind::Image => {
            let key = match provider.as_str() {
                "nanobanana" | "gemini" | "google" => &config.api_key,
                "dalle" | "openai" => &config.openai_api_key,
                "stability" => &config.stability_api_key,
                "ideogram" => &config.ideogram_api_key,
                "replicate" => &config.replicate_api_key,
                "bfl" | "flux" => &config.bfl_api_key,
                _ => return Vec::new(),
            };
            (
                key,
                super::image_models::image_model_options_for_provider(config, &provider),
            )
        }
        ModelKind::Video => match provider.as_str() {
            "veo" | "gemini" | "google" => (
                &config.api_key,
                super::video_models::video_model_options_for_provider(config, &provider),
            ),
            _ => return Vec::new(),
        },
        ModelKind::Live => (
            &config.api_key,
            GEMINI_LIVE_MODEL_PRESETS
                .iter()
                .map(|model| (*model).to_owned())
                .collect(),
        ),
    };
    let api_key = if api_key_override.trim().is_empty() {
        saved_key
    } else {
        api_key_override
    };
    if api_key.trim().is_empty() {
        return presets;
    }
    let mut models = discover(kind, provider, api_key.to_owned()).await;
    for preset in presets {
        if !models.contains(&preset) {
            models.push(preset);
        }
    }
    models
}

// Only provider IO varies at this internal seam; credential and fallback policy
// stay in the catalog. Fetchers retain their existing timeout, parsing and cache.
async fn discover(kind: ModelKind, provider: String, api_key: String) -> Vec<String> {
    match kind {
        ModelKind::Image => {
            super::image_model_fetcher::fetch_image_provider_models(&provider, &api_key).await
        }
        ModelKind::Video => {
            super::video_model_fetcher::fetch_video_provider_models(&provider, &api_key).await
        }
        ModelKind::Live => crate::slash::model_fetcher::fetch_gemini_live_models(&api_key).await,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn configured_keys() -> MintConfig {
        MintConfig {
            api_key: "saved-google".into(),
            openai_api_key: "saved-openai".into(),
            stability_api_key: "saved-stability".into(),
            ideogram_api_key: "saved-ideogram".into(),
            replicate_api_key: "saved-replicate".into(),
            bfl_api_key: "saved-bfl".into(),
            ..MintConfig::default()
        }
    }

    #[tokio::test]
    async fn image_discovery_keeps_presets_without_repeating_a_discovered_preset() {
        let models = models_with_discovery(
            &configured_keys(),
            ModelKind::Image,
            "openai",
            "",
            |_, _, _| async { vec!["dall-e-3".into(), "new-image-model".into()] },
        )
        .await;
        assert_eq!(
            models,
            ["dall-e-3", "new-image-model", "gpt-image-1", "dall-e-2"]
        );
    }

    #[tokio::test]
    async fn image_aliases_use_their_own_saved_credentials() {
        for (provider, normalized, key) in [
            (" NanoBanana ", "nanobanana", "saved-google"),
            (" GEMINI ", "gemini", "saved-google"),
            (" Google ", "google", "saved-google"),
            (" Dalle ", "dalle", "saved-openai"),
            (" OpenAI ", "openai", "saved-openai"),
            (" Stability ", "stability", "saved-stability"),
            (" Ideogram ", "ideogram", "saved-ideogram"),
            (" Replicate ", "replicate", "saved-replicate"),
            (" BFL ", "bfl", "saved-bfl"),
            (" Flux ", "flux", "saved-bfl"),
        ] {
            let models = models_with_discovery(
                &configured_keys(),
                ModelKind::Image,
                provider,
                "",
                |kind, actual_provider, actual_key| async move {
                    if matches!(kind, ModelKind::Image)
                        && actual_provider == normalized
                        && actual_key == key
                    {
                        vec!["authorized-image-model".into()]
                    } else {
                        Vec::new()
                    }
                },
            )
            .await;
            assert_eq!(
                models.first().map(String::as_str),
                Some("authorized-image-model"),
                "{provider}"
            );
        }
    }

    #[tokio::test]
    async fn explicit_key_overrides_saved_credentials_for_every_catalog() {
        for (kind, provider) in [
            (ModelKind::Image, "openai"),
            (ModelKind::Video, "veo"),
            (ModelKind::Live, "gemini"),
        ] {
            let models = models_with_discovery(
                &configured_keys(),
                kind,
                provider,
                "unsaved-key",
                |_, _, key| async move {
                    if key == "unsaved-key" {
                        vec!["override-model".into()]
                    } else {
                        Vec::new()
                    }
                },
            )
            .await;
            assert_eq!(models.first().map(String::as_str), Some("override-model"));
        }
    }

    #[tokio::test]
    async fn blank_override_uses_saved_google_key_for_video_aliases_and_live() {
        for (kind, provider) in [
            (ModelKind::Video, " VEO "),
            (ModelKind::Video, " GOOGLE "),
            (ModelKind::Video, " GEMINI "),
            (ModelKind::Live, "gemini"),
        ] {
            let models = models_with_discovery(
                &configured_keys(),
                kind,
                provider,
                " \t\n",
                |_, _, key| async move {
                    if key == "saved-google" {
                        vec!["saved-key-model".into()]
                    } else {
                        Vec::new()
                    }
                },
            )
            .await;
            assert_eq!(models.first().map(String::as_str), Some("saved-key-model"));
        }
    }

    #[tokio::test]
    async fn video_discovery_keeps_missing_presets_after_discovered_models() {
        let models = models_with_discovery(
            &configured_keys(),
            ModelKind::Video,
            "veo",
            "",
            |_, _, _| async {
                vec![
                    "new-video-model".into(),
                    "veo-3.1-fast-generate-preview".into(),
                ]
            },
        )
        .await;
        assert_eq!(
            models,
            [
                "new-video-model",
                "veo-3.1-fast-generate-preview",
                "veo-3.1-generate-preview",
                "veo-3.1-lite-generate-preview",
                "veo-2.0-generate-001"
            ]
        );
    }

    #[tokio::test]
    async fn live_discovery_keeps_the_existing_fallback_model_selectable() {
        let models = models_with_discovery(
            &configured_keys(),
            ModelKind::Live,
            "gemini",
            "",
            |_, _, _| async {
                vec![
                    "new-live-model".into(),
                    "gemini-3.1-flash-live-preview".into(),
                ]
            },
        )
        .await;
        assert_eq!(
            models,
            [
                "new-live-model",
                "gemini-3.1-flash-live-preview",
                "gemini-2.5-flash-native-audio-preview-12-2025"
            ]
        );
    }

    #[tokio::test]
    async fn failed_discovery_returns_presets_even_with_an_invalid_override_key() {
        for (kind, provider, expected_first) in [
            (ModelKind::Image, "openai", "gpt-image-1"),
            (ModelKind::Video, "veo", "veo-3.1-generate-preview"),
            (
                ModelKind::Live,
                "gemini",
                "gemini-2.5-flash-native-audio-preview-12-2025",
            ),
        ] {
            let models = models_with_discovery(
                &MintConfig::default(),
                kind,
                provider,
                "invalid",
                |_, _, _| async { Vec::new() },
            )
            .await;
            assert_eq!(models.first().map(String::as_str), Some(expected_first));
        }
    }

    #[tokio::test]
    async fn missing_or_whitespace_saved_keys_return_presets_without_discovery() {
        let config = MintConfig {
            api_key: " \t".into(),
            ..MintConfig::default()
        };
        for (kind, provider, expected_first) in [
            (ModelKind::Image, "openai", "gpt-image-1"),
            (ModelKind::Video, "veo", "veo-3.1-generate-preview"),
            (
                ModelKind::Live,
                "gemini",
                "gemini-2.5-flash-native-audio-preview-12-2025",
            ),
        ] {
            let models = models_with_discovery(&config, kind, provider, "", |_, _, _| async {
                panic!("must not contact the provider without credentials")
            })
            .await;
            assert_eq!(models.first().map(String::as_str), Some(expected_first));
        }
    }

    #[tokio::test]
    async fn unsupported_providers_do_not_offer_models_from_another_provider() {
        for kind in [ModelKind::Image, ModelKind::Video] {
            let models = models_with_discovery(
                &configured_keys(),
                kind,
                "unknown",
                "override",
                |_, _, _| async { panic!("unsupported provider must not start discovery") },
            )
            .await;
            assert!(models.is_empty());
        }
    }
}

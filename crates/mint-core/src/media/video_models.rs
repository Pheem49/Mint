//! Video generation provider → model presets and live model discovery.
//!
//! Provides static fallback presets and [`video_model_options_for_provider_async`],
//! which calls provider APIs before falling back to presets.

use crate::MintConfig;

pub const VEO_VIDEO_MODEL_PRESETS: &[&str] = &[
    "veo-3.1-generate-preview",
    "veo-3.1-fast-generate-preview",
    "veo-3.1-lite-generate-preview",
    "veo-2.0-generate-001",
];

/// Display label for each video generation provider.
pub fn video_provider_display_name(provider: &str) -> String {
    match provider.trim().to_lowercase().as_str() {
        "veo" | "gemini" | "google" => "Google Veo (Gemini Videos)".into(),
        other => other.to_string(),
    }
}

/// Static model preset list for a video provider.
pub fn video_model_options_for_provider(_config: &MintConfig, provider: &str) -> Vec<String> {
    match provider.trim().to_lowercase().as_str() {
        "veo" | "gemini" | "google" => VEO_VIDEO_MODEL_PRESETS,
        _ => &[],
    }
    .iter()
    .map(|s| s.to_string())
    .collect()
}

/// Tries the provider's live API first to fetch available video models,
/// falling back to static presets on network error, timeout, or missing key.
pub async fn video_model_options_for_provider_async(
    config: &MintConfig,
    provider: &str,
) -> Vec<String> {
    use super::video_model_fetcher;

    let lower = provider.trim().to_lowercase();
    let api_key = match lower.as_str() {
        "veo" | "gemini" | "google" => &config.api_key,
        _ => return video_model_options_for_provider(config, provider),
    };

    let dynamic = video_model_fetcher::fetch_video_provider_models(&lower, api_key).await;
    if !dynamic.is_empty() {
        let mut merged = dynamic;
        for preset in video_model_options_for_provider(config, provider) {
            if !merged.contains(&preset) {
                merged.push(preset);
            }
        }
        merged
    } else {
        video_model_options_for_provider(config, provider)
    }
}

/// Get the active model for a video provider from config.
pub fn active_video_model_for_provider<'a>(config: &'a MintConfig, provider: &str) -> &'a str {
    match provider.trim().to_lowercase().as_str() {
        "veo" | "gemini" | "google" => config
            .extra
            .get("veoModel")
            .and_then(|v| v.as_str())
            .unwrap_or("veo-3.1-generate-preview"),
        _ => "",
    }
}

/// Sets the active video model for `provider` in `config`.
pub fn set_active_video_provider_model(
    config: &mut MintConfig,
    provider: &str,
    model: Option<&str>,
) {
    let lower = provider.trim().to_lowercase();
    let canonical = match lower.as_str() {
        "gemini" | "google" => "veo",
        other => other,
    };
    config.extra.insert(
        "videoGenProvider".to_string(),
        serde_json::Value::String(canonical.to_string()),
    );

    if let Some(m) = model.filter(|s| !s.trim().is_empty()) {
        match canonical {
            "veo" => {
                config.extra.insert(
                    "veoModel".to_string(),
                    serde_json::Value::String(m.to_string()),
                );
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_video_models_case_insensitivity() {
        let cfg = MintConfig::default();
        let opts = video_model_options_for_provider(&cfg, "Veo");
        assert_eq!(opts, VEO_VIDEO_MODEL_PRESETS);

        let opts_gemini = video_model_options_for_provider(&cfg, "Gemini");
        assert_eq!(opts_gemini, VEO_VIDEO_MODEL_PRESETS);

        assert_eq!(
            video_provider_display_name("veo"),
            "Google Veo (Gemini Videos)"
        );
        assert_eq!(
            video_provider_display_name("Google"),
            "Google Veo (Gemini Videos)"
        );
    }

    #[test]
    fn test_set_and_get_active_video_model() {
        let mut cfg = MintConfig::default();
        assert_eq!(
            active_video_model_for_provider(&cfg, "veo"),
            "veo-3.1-generate-preview"
        );

        set_active_video_provider_model(&mut cfg, "veo", Some("veo-3.1-fast-generate-preview"));
        assert_eq!(
            active_video_model_for_provider(&cfg, "veo"),
            "veo-3.1-fast-generate-preview"
        );
        assert_eq!(
            cfg.extra.get("videoGenProvider").and_then(|v| v.as_str()),
            Some("veo")
        );
    }
}

//! Image generation provider → model presets and live model discovery.
//!
//! Provides static fallback presets and [`image_model_options_for_provider_async`],
//! which calls provider APIs before falling back to presets.

use crate::MintConfig;

pub const NANOBANANA_IMAGE_MODEL_PRESETS: &[&str] = &[
    "gemini-3.1-flash-image",
    "gemini-3-pro-image",
    "gemini-2.5-flash-image",
];

pub const DALLE_MODEL_PRESETS: &[&str] = &["gpt-image-1", "dall-e-3", "dall-e-2"];

pub const STABILITY_MODEL_PRESETS: &[&str] = &[
    "ultra",
    "core",
    "sd3.5-large",
    "sd3.5-large-turbo",
    "sd3-medium",
];

pub const IDEOGRAM_MODEL_PRESETS: &[&str] = &["V_3", "V_2", "V_2_TURBO"];

pub const REPLICATE_MODEL_PRESETS: &[&str] = &[
    "black-forest-labs/flux-1.1-pro",
    "black-forest-labs/flux-kontext-pro",
    "black-forest-labs/flux-fill-pro",
    "black-forest-labs/flux-schnell",
    "stability-ai/sdxl",
    "timbrooks/instruct-pix2pix",
];

pub const BFL_MODEL_PRESETS: &[&str] = &[
    "flux-pro-1.1",
    "flux-pro-1.1-ultra",
    "flux-pro",
    "flux-dev",
    "flux-schnell",
    "flux-kontext-pro",
    "flux-kontext-max",
    "flux-fill-pro",
];

/// Display label for each image provider.
pub fn image_provider_display_name(provider: &str) -> String {
    match provider.trim().to_lowercase().as_str() {
        "nanobanana" | "gemini" | "google" => "NanoBanana (Gemini)".into(),
        "dalle" | "openai" => "DALL·E (OpenAI)".into(),
        "stability" => "Stability AI".into(),
        "ideogram" => "Ideogram".into(),
        "replicate" => "Replicate".into(),
        "bfl" | "flux" => "Black Forest Labs (FLUX)".into(),
        other => other.to_string(),
    }
}

/// Static model preset list for an image provider.
pub fn image_model_options_for_provider(_config: &MintConfig, provider: &str) -> Vec<String> {
    match provider.trim().to_lowercase().as_str() {
        "nanobanana" | "gemini" | "google" => NANOBANANA_IMAGE_MODEL_PRESETS,
        "dalle" | "openai" => DALLE_MODEL_PRESETS,
        "stability" => STABILITY_MODEL_PRESETS,
        "ideogram" => IDEOGRAM_MODEL_PRESETS,
        "replicate" => REPLICATE_MODEL_PRESETS,
        "bfl" | "flux" => BFL_MODEL_PRESETS,
        _ => &[],
    }
    .iter()
    .map(|s| s.to_string())
    .collect()
}

/// Tries the provider's live API first to fetch available image models,
/// falling back to static presets on network error, timeout, or missing key.
pub async fn image_model_options_for_provider_async(
    config: &MintConfig,
    provider: &str,
) -> Vec<String> {
    use super::image_model_fetcher;

    let lower = provider.trim().to_lowercase();
    let api_key = match lower.as_str() {
        "nanobanana" | "gemini" | "google" => &config.api_key,
        "dalle" | "openai" => &config.openai_api_key,
        "stability" => &config.stability_api_key,
        "ideogram" => &config.ideogram_api_key,
        "replicate" => &config.replicate_api_key,
        "bfl" | "flux" => &config.bfl_api_key,
        _ => return image_model_options_for_provider(config, provider),
    };

    let dynamic = image_model_fetcher::fetch_image_provider_models(&lower, api_key).await;
    if !dynamic.is_empty() {
        let mut merged = dynamic;
        for preset in image_model_options_for_provider(config, provider) {
            if !merged.contains(&preset) {
                merged.push(preset);
            }
        }
        merged
    } else {
        image_model_options_for_provider(config, provider)
    }
}

/// Get the active model for an image provider from config.
pub fn active_image_model_for_provider<'a>(config: &'a MintConfig, provider: &str) -> &'a str {
    match provider.trim().to_lowercase().as_str() {
        "nanobanana" | "gemini" | "google" => &config.nanobanana_model,
        "dalle" | "openai" => &config.dalle_model,
        "stability" => &config.stability_model,
        "ideogram" => &config.ideogram_model,
        "replicate" => &config.replicate_model,
        "bfl" | "flux" => &config.bfl_model,
        _ => "",
    }
}

/// Sets the active image model for `provider` in `config`.
pub fn set_active_image_provider_model(
    config: &mut MintConfig,
    provider: &str,
    model: Option<&str>,
) {
    let lower = provider.trim().to_lowercase();
    let canonical = match lower.as_str() {
        "gemini" | "google" => "nanobanana",
        "flux" => "bfl",
        "openai" => "dalle",
        other => other,
    };
    config.image_gen_provider = canonical.to_string();

    if let Some(m) = model.filter(|s| !s.trim().is_empty()) {
        match canonical {
            "nanobanana" => config.nanobanana_model = m.to_string(),
            "dalle" => config.dalle_model = m.to_string(),
            "stability" => config.stability_model = m.to_string(),
            "ideogram" => config.ideogram_model = m.to_string(),
            "replicate" => config.replicate_model = m.to_string(),
            "bfl" => config.bfl_model = m.to_string(),
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_image_models_case_insensitivity() {
        let cfg = MintConfig::default();
        let opts = image_model_options_for_provider(&cfg, "Gemini");
        assert_eq!(opts, NANOBANANA_IMAGE_MODEL_PRESETS);

        let opts = image_model_options_for_provider(&cfg, "DALLE");
        assert_eq!(opts, DALLE_MODEL_PRESETS);

        let opts = image_model_options_for_provider(&cfg, "FLUX");
        assert_eq!(opts, BFL_MODEL_PRESETS);

        assert_eq!(image_provider_display_name("gemini"), "NanoBanana (Gemini)");
        assert_eq!(image_provider_display_name("OpenAI"), "DALL·E (OpenAI)");
    }
}

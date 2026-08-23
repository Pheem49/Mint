use super::*;

pub fn format_provider_display_name(provider: &str, config: &mint_core::MintConfig) -> String {
    if let Some(custom) = config.resolve_custom_provider(provider) {
        custom.display_name.clone()
    } else {
        match provider {
            "gemini" => "Google Gemini".to_owned(),
            "anthropic" => "Anthropic Claude".to_owned(),
            "openai" => "OpenAI".to_owned(),
            "openrouter" => "OpenRouter".to_owned(),
            "deepseek" => "DeepSeek".to_owned(),
            "huggingface" => "Hugging Face".to_owned(),
            "local_openai" => "Local OpenAI".to_owned(),
            "ollama" => "Ollama".to_owned(),
            p => p.to_owned(),
        }
    }
}

/// Renders a stored UTC RFC3339 timestamp (e.g. a cron job's `next_run`) in
/// the local system timezone with an explicit offset, so `/cron list` shown
/// in a terminal reads the same "when" a human means as the web/desktop
/// app's already-localized display, instead of a raw UTC instant that looks
/// like a different time even though it isn't. Falls back to the raw string
/// if it doesn't parse as RFC3339 (defensive only — cron timestamps are
/// always written by `CronStore` in that format).
pub fn format_local_time(rfc3339: &str) -> String {
    match chrono::DateTime::parse_from_rfc3339(rfc3339) {
        Ok(utc_time) => utc_time
            .with_timezone(&chrono::Local)
            .format("%Y-%m-%d %H:%M %:z")
            .to_string(),
        Err(_) => rfc3339.to_string(),
    }
}
pub fn format_path_with_tilde(path: &Path) -> String {
    let path_str = path.to_string_lossy().to_string();
    if let Some(home) = dirs::home_dir() {
        let home_str = home.to_string_lossy().to_string();
        if path_str.starts_with(&home_str) {
            return path_str.replacen(&home_str, "~", 1);
        }
    }
    path_str
}
/// Placeholder prefixes that get colored when they appear in the input box —
/// currently `[Pasted text #N]`/`[Pasted text #N +K lines]` (inserted by
/// large-paste detection) and `[Image #N]` (inserted by Ctrl+V image paste).
/// Both are literal markers the input box itself writes into the buffer, so
/// matching on a fixed prefix is safe — user-typed text starting with the
/// same characters is vanishingly unlikely and, worst case, only affects
/// display color, not the text actually sent.
const PLACEHOLDER_PREFIXES: [&str; 2] = ["[Pasted text #", "[Image #"];

fn matches_placeholder_prefix(chars: &[char], i: usize) -> bool {
    PLACEHOLDER_PREFIXES.iter().any(|prefix| {
        let prefix_chars: Vec<char> = prefix.chars().collect();
        chars[i..].starts_with(&prefix_chars)
    })
}

pub fn format_placeholders(s: &str) -> String {
    let mut result = String::new();
    let chars = s.chars().collect::<Vec<_>>();
    let mut i = 0;
    while i < chars.len() {
        if chars[i] == '['
            && matches_placeholder_prefix(&chars, i)
            && let Some(end_offset) = chars[i..].iter().position(|&c| c == ']')
        {
            let end_idx = i + end_offset;
            let inside: String = chars[i + 1..end_idx].iter().collect();
            result.push('[');
            result.push_str(BLUE);
            result.push_str(&inside);
            result.push_str("\x1b[39m");
            result.push(']');
            i = end_idx + 1;
            continue;
        }
        result.push(chars[i]);
        i += 1;
    }
    result
}
pub fn is_thai_zero_width(c: char) -> bool {
    let cp = c as u32;
    cp == 0x0E31 || (0x0E34..=0x0E3A).contains(&cp) || (0x0E47..=0x0E4E).contains(&cp)
}
pub fn char_visual_width(c: char) -> usize {
    if is_thai_zero_width(c) { 0 } else { 1 }
}
pub fn string_visual_width(s: &str) -> usize {
    s.chars().map(char_visual_width).sum()
}
pub fn active_model<'a>(provider: &str, config: &'a mint_core::MintConfig) -> &'a str {
    match provider {
        "anthropic" => &config.anthropic_model,
        "openai" => &config.openai_model,
        "openrouter" => &config.openrouter_model,
        "deepseek" => &config.deepseek_model,
        "huggingface" => &config.hf_model,
        "local_openai" => &config.local_model_name,
        "ollama" => &config.ollama_model,
        p if p.starts_with("custom:") => {
            if let Some(id) = p.strip_prefix("custom:")
                && let Some(selections) = config
                    .extra
                    .get("customModelSelections")
                    .and_then(|v| v.as_object())
                && let Some(m) = selections.get(id).and_then(|v| v.as_str())
            {
                return m;
            }
            config
                .resolve_custom_provider(p)
                .and_then(|cp| cp.models.first())
                .map(|m| m.model_id.as_str())
                .unwrap_or("")
        }
        _ => &config.gemini_model,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_placeholders_colors_pasted_text() {
        let out = format_placeholders("hi [Pasted text #1 +5 lines] end");
        assert!(out.contains(BLUE));
        assert!(out.contains("Pasted text #1 +5 lines"));
    }

    #[test]
    fn format_placeholders_colors_image() {
        let out = format_placeholders("[Pasted text #1] [Image #1]");
        // Both placeholders get their own BLUE...reset pair — two of each.
        assert_eq!(out.matches(BLUE).count(), 2);
        assert!(out.contains("Image #1"));
    }

    #[test]
    fn format_placeholders_leaves_plain_text_untouched() {
        assert_eq!(
            format_placeholders("just typing normally"),
            "just typing normally"
        );
    }
}

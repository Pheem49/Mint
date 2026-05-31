use serde::Serialize;

const MAX_GOOGLE_TTS_CHARS: usize = 200;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TtsUrl {
    pub short_text: String,
    pub url: String,
}

pub fn google_tts_urls(text: &str, language: &str) -> Vec<TtsUrl> {
    let chunks = split_text(text, MAX_GOOGLE_TTS_CHARS);
    let total = chunks.len();
    chunks
        .into_iter()
        .enumerate()
        .map(|(index, chunk)| TtsUrl {
            url: format!(
                "https://translate.google.com/translate_tts?ie=UTF-8&q={}&tl={}&client=tw-ob&idx={index}&total={total}&textlen={}",
                encode_component(&chunk),
                encode_component(language),
                chunk.chars().count()
            ),
            short_text: chunk,
        })
        .collect()
}

fn split_text(text: &str, max_length: usize) -> Vec<String> {
    let mut remaining = text.split_whitespace().collect::<Vec<_>>().join(" ");
    let mut chunks = Vec::new();
    while remaining.chars().count() > max_length {
        let char_boundary = remaining
            .char_indices()
            .nth(max_length)
            .map(|(index, _)| index)
            .unwrap_or(remaining.len());
        let boundary = remaining
            .char_indices()
            .take_while(|(index, _)| *index <= char_boundary)
            .filter(|(_, character)| matches!(character, '.' | '?' | '!' | ',' | ' '))
            .map(|(index, _)| index)
            .last()
            .filter(|index| *index > 0)
            .unwrap_or(char_boundary);
        chunks.push(remaining[..boundary].trim().to_owned());
        remaining = remaining[boundary..].trim().to_owned();
    }
    if !remaining.is_empty() {
        chunks.push(remaining);
    }
    chunks
}

fn encode_component(value: &str) -> String {
    value
        .bytes()
        .map(|byte| match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                (byte as char).to_string()
            }
            b' ' => "+".into(),
            _ => format!("%{byte:02X}"),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splits_long_text_for_google_tts() {
        let chunks = split_text(&"a ".repeat(150), MAX_GOOGLE_TTS_CHARS);
        assert!(chunks.len() > 1);
        assert!(
            chunks
                .iter()
                .all(|chunk| chunk.len() <= MAX_GOOGLE_TTS_CHARS)
        );
    }

    #[test]
    fn encodes_tts_query() {
        assert!(
            google_tts_urls("hello mint", "en")[0]
                .url
                .contains("q=hello+mint")
        );
    }
}

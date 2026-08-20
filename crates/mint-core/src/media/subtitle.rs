use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::config::MintConfig;
use crate::speech::TranscriptSegment;
use crate::video_edit::{VideoEditResult, check_ffmpeg, run_ffmpeg_args, video_load};

#[derive(Debug, Error)]
pub enum SubtitleError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("ffmpeg error: {0}")]
    Ffmpeg(String),
    #[error("API or LLM translation error: {0}")]
    TranslationFailed(String),
    #[error("invalid SRT format: {0}")]
    InvalidSrt(String),
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
}

fn temp_id() -> String {
    uuid::Uuid::new_v4().simple().to_string()
}

/// Style settings for burning subtitles into video.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SubtitleStyle {
    /// Font name e.g. "Arial", "Roboto", "Impact".
    #[serde(default = "default_font")]
    pub font_name: String,
    /// Font size in points (default: 24).
    #[serde(default = "default_font_size")]
    pub font_size: u32,
    /// Primary text color in ASS hex format e.g. "&H00FFFFFF" (White) or "&H0000FFFF" (Yellow).
    #[serde(default = "default_primary_color")]
    pub primary_color: String,
    /// Outline color in ASS hex format e.g. "&H00000000" (Black).
    #[serde(default = "default_outline_color")]
    pub outline_color: String,
    /// Outline thickness (0-4).
    #[serde(default = "default_outline")]
    pub outline: u32,
    /// Alignment (2 = Bottom Center, 6 = Top Center, 10 = Mid Center).
    #[serde(default = "default_alignment")]
    pub alignment: u32,
    /// Vertical margin from bottom in pixels (default: 30).
    #[serde(default = "default_margin_v")]
    pub margin_v: u32,
}

fn default_font() -> String {
    "Arial".to_string()
}
fn default_font_size() -> u32 {
    24
}
fn default_primary_color() -> String {
    "&H00FFFFFF".to_string()
}
fn default_outline_color() -> String {
    "&H00000000".to_string()
}
fn default_outline() -> u32 {
    2
}
fn default_alignment() -> u32 {
    2
}
fn default_margin_v() -> u32 {
    30
}

impl Default for SubtitleStyle {
    fn default() -> Self {
        Self {
            font_name: default_font(),
            font_size: default_font_size(),
            primary_color: default_primary_color(),
            outline_color: default_outline_color(),
            outline: default_outline(),
            alignment: default_alignment(),
            margin_v: default_margin_v(),
        }
    }
}

impl SubtitleStyle {
    /// Preset: TikTok-style bold yellow text with heavy black border.
    pub fn tiktok() -> Self {
        Self {
            font_name: "Impact".into(),
            font_size: 28,
            primary_color: "&H0000FFFF".into(), // Yellow
            outline_color: "&H00000000".into(), // Black
            outline: 3,
            alignment: 2,
            margin_v: 60,
        }
    }

    /// Preset: Minimalist clean white text with dark outline.
    pub fn minimal() -> Self {
        Self {
            font_name: "Arial".into(),
            font_size: 22,
            primary_color: "&H00FFFFFF".into(),
            outline_color: "&H00000000".into(),
            outline: 1,
            alignment: 2,
            margin_v: 25,
        }
    }

    /// Build FFmpeg `force_style` string.
    pub fn to_force_style_str(&self) -> String {
        format!(
            "FontName={},FontSize={},PrimaryColour={},OutlineColour={},Outline={},Alignment={},MarginV={}",
            self.font_name,
            self.font_size,
            self.primary_color,
            self.outline_color,
            self.outline,
            self.alignment,
            self.margin_v
        )
    }
}

/// Request for burning subtitles into video.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BurnSubtitleRequest {
    pub input_video: String,
    /// Path to .srt file, or raw SRT string content.
    pub srt_input: String,
    pub output_video: String,
    #[serde(default)]
    pub style: Option<SubtitleStyle>,
    #[serde(default)]
    pub preset: Option<String>,
}

/// Request for translating subtitles.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TranslateSubtitleRequest {
    pub srt_content: String,
    pub target_language: String,
}

// ── Functions ──────────────────────────────────────────────────────────────

use srtlib::{Subtitle, Subtitles, Timestamp};

/// Convert seconds to `srtlib::Timestamp`.
fn secs_to_timestamp(secs: f64) -> Timestamp {
    let total_ms = (secs.max(0.0) * 1000.0).round() as u64;
    let hours = (total_ms / 3_600_000).min(255) as u8;
    let mins = ((total_ms % 3_600_000) / 60_000) as u8;
    let s = ((total_ms % 60_000) / 1_000) as u8;
    let ms = (total_ms % 1_000) as u16;
    Timestamp::new(hours, mins, s, ms)
}

/// Convert `TranscriptSegment` slice into a valid SRT format string.
pub fn generate_srt(segments: &[TranscriptSegment]) -> String {
    let mut subtitles = Subtitles::new();
    for (i, seg) in segments.iter().enumerate() {
        let start_ts = secs_to_timestamp(seg.start);
        let end_ts = secs_to_timestamp(seg.end);
        let text = if let Some(speaker) = &seg.speaker {
            format!("{}: {}", speaker, seg.text)
        } else {
            seg.text.clone()
        };
        subtitles.push(Subtitle::new(i + 1, start_ts, end_ts, text));
    }
    subtitles.to_string()
}

/// Convert seconds to SRT timecode (`HH:MM:SS,mmm`).
pub fn secs_to_srt_timestamp(secs: f64) -> String {
    secs_to_timestamp(secs).to_string()
}

/// Burn subtitles into a video file using FFmpeg filter.
pub fn burn_subtitles(req: &BurnSubtitleRequest) -> Result<VideoEditResult, SubtitleError> {
    check_ffmpeg().map_err(|e| SubtitleError::Ffmpeg(e.to_string()))?;

    // Determine SRT file path
    let srt_path =
        if req.srt_input.ends_with(".srt") && std::path::Path::new(&req.srt_input).exists() {
            req.srt_input.clone()
        } else {
            // Write raw content to temp file
            let tmp_file = std::env::temp_dir().join(format!("mint_burn_sub_{}.srt", temp_id()));
            std::fs::write(&tmp_file, &req.srt_input)?;
            tmp_file.to_string_lossy().to_string()
        };

    // Determine style
    let style = if let Some(preset) = &req.preset {
        match preset.to_lowercase().as_str() {
            "tiktok" => SubtitleStyle::tiktok(),
            "minimal" => SubtitleStyle::minimal(),
            _ => req.style.clone().unwrap_or_default(),
        }
    } else {
        req.style.clone().unwrap_or_default()
    };

    let force_style = style.to_force_style_str();
    let escaped_srt = srt_path.replace('\\', "/").replace(':', "\\:");
    let filter = format!("subtitles='{}':force_style='{}'", escaped_srt, force_style);

    run_ffmpeg_args(&[
        "-y",
        "-i",
        &req.input_video,
        "-vf",
        &filter,
        "-c:a",
        "copy",
        &req.output_video,
    ])
    .map_err(|e| SubtitleError::Ffmpeg(e.to_string()))?;

    let info = video_load(&req.output_video).ok();
    Ok(VideoEditResult {
        output_path: req.output_video.clone(),
        duration: info.as_ref().map(|v| v.duration),
        size_bytes: info.as_ref().map(|v| v.size_bytes),
    })
}

/// Translate SRT subtitle content to `target_language`.
pub async fn translate_subtitles(
    config: &MintConfig,
    req: &TranslateSubtitleRequest,
) -> Result<String, SubtitleError> {
    if req.srt_content.trim().is_empty() {
        return Ok(String::new());
    }

    // Call LLM for translation if API key available
    if !config.api_key.trim().is_empty() || !config.openai_api_key.trim().is_empty() {
        let system_prompt = format!(
            "You are an expert video subtitler. Translate the text lines in the following SRT subtitle file into {}. \
            IMPORTANT: Preserve all subtitle index numbers and timestamp ranges EXACTLY as they are. Only translate the text content below each timestamp range.",
            req.target_language
        );

        let prompt = format!("{}\n\nSRT Content:\n{}", system_prompt, req.srt_content);

        let chat_req = crate::chat::ChatRequest {
            message: prompt,
            system_instruction: "You translate SRT subtitles accurately.".into(),
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
        };

        match crate::chat::send_chat_with_fallback(config, &chat_req).await {
            Ok((res, _)) => return Ok(res.text),
            Err(e) => eprintln!("[mint-subtitle] LLM translation error: {e}, falling back..."),
        }
    }

    // Fallback: append language code prefix if no LLM configured
    let mut translated = String::new();
    for line in req.srt_content.lines() {
        if line.contains("-->") || line.trim().parse::<u32>().is_ok() || line.trim().is_empty() {
            translated.push_str(line);
        } else {
            translated.push_str(&format!(
                "[{}] {}",
                req.target_language.to_uppercase(),
                line
            ));
        }
        translated.push('\n');
    }
    Ok(translated)
}

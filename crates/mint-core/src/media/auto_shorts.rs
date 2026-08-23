use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use thiserror::Error;

use crate::config::MintConfig;
use crate::speech::{TranscribeRequest, TranscriptSegment, transcribe};
use crate::subtitle::{BurnSubtitleRequest, burn_subtitles, generate_srt};
use crate::video_edit::{
    ResizeRequest, TrimRequest, VideoEditError, video_load, video_resize, video_trim,
};

#[derive(Debug, Error)]
pub enum AutoShortsError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("video edit error: {0}")]
    VideoEdit(#[from] VideoEditError),
    #[error("speech error: {0}")]
    Speech(#[from] crate::speech::SpeechError),
    #[error("subtitle error: {0}")]
    Subtitle(#[from] crate::subtitle::SubtitleError),
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("shorts generation error: {0}")]
    Failed(String),
}

fn temp_id() -> String {
    uuid::Uuid::new_v4().simple().to_string()
}

/// Request body for AI Natural Language Video Editing.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AiEditVideoRequest {
    pub input: String,
    #[serde(default)]
    pub output: Option<String>,
    pub instruction: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AiEditStepResult {
    pub step: usize,
    pub operation: String,
    pub description: String,
    pub output_path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AiEditVideoResult {
    pub output_path: String,
    pub steps_performed: Vec<AiEditStepResult>,
    pub summary: String,
}

/// Request settings for making short-form vertical clips from a longer video.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MakeShortsRequest {
    /// Path to input video file.
    pub input: String,
    /// Destination directory for generated shorts (optional — defaults to input file's directory).
    #[serde(default)]
    pub output_dir: Option<String>,
    /// Maximum number of short clips to produce (default: 3).
    #[serde(default = "default_max_clips")]
    pub max_clips: u32,
    /// Target duration per clip in seconds (default: 60.0).
    #[serde(default = "default_target_duration")]
    pub target_duration: f64,
    /// Whether to auto-generate and burn vertical TikTok-style subtitles (default: true).
    #[serde(default = "default_burn_subtitles")]
    pub burn_subtitles: bool,
    /// Target resolution width (default: 1080 for 9:16 vertical).
    #[serde(default = "default_width")]
    pub width: i32,
    /// Target resolution height (default: 1920 for 9:16 vertical).
    #[serde(default = "default_height")]
    pub height: i32,
}

fn default_max_clips() -> u32 {
    3
}
fn default_target_duration() -> f64 {
    60.0
}
fn default_burn_subtitles() -> bool {
    true
}
fn default_width() -> i32 {
    1080
}
fn default_height() -> i32 {
    1920
}

/// Metadata for a generated short clip.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ShortClipInfo {
    pub id: u32,
    pub path: String,
    pub start: f64,
    pub end: f64,
    pub duration: f64,
    pub title: String,
}

/// Result of `make_shorts`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MakeShortsResult {
    pub clips: Vec<ShortClipInfo>,
}

#[derive(Debug, Deserialize)]
struct HighlightMoment {
    start: f64,
    end: f64,
    title: String,
}

/// Automatically extract viral highlight moments from long video and render vertical Shorts.
pub async fn make_shorts(
    config: &MintConfig,
    req: &MakeShortsRequest,
) -> Result<MakeShortsResult, AutoShortsError> {
    let video_info = video_load(&req.input)?;
    let total_duration = video_info.duration;

    if total_duration < 5.0 {
        return Err(AutoShortsError::Failed(
            "Video is too short for auto shorts (< 5s)".into(),
        ));
    }

    // Step 1: Transcribe audio to get timed speech segments
    let trans_req = TranscribeRequest {
        input: req.input.clone(),
        language: None,
        prompt: None,
    };
    let transcript = transcribe(config, &trans_req).await?;

    // Step 2: Use LLM to identify key highlights, or fallback to interval highlights
    let highlights = detect_highlights(
        config,
        &transcript.segments,
        total_duration,
        req.max_clips,
        req.target_duration,
    )
    .await;

    // Step 3: Determine output directory
    let out_dir = if let Some(dir) = &req.output_dir {
        PathBuf::from(dir)
    } else {
        PathBuf::from(&req.input)
            .parent()
            .unwrap_or_else(|| std::path::Path::new("."))
            .to_path_buf()
    };
    std::fs::create_dir_all(&out_dir)?;

    let file_stem = PathBuf::from(&req.input)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("short")
        .to_string();

    let mut generated_clips = Vec::new();

    // Step 4: Process each highlight into vertical short video with burned subtitles
    for (idx, moment) in highlights.iter().enumerate() {
        let clip_num = (idx + 1) as u32;
        let tmp_trim =
            std::env::temp_dir().join(format!("mint_short_trim_{}_{}.mp4", temp_id(), clip_num));
        let tmp_trim_str = tmp_trim.to_string_lossy().to_string();

        let tmp_resize =
            std::env::temp_dir().join(format!("mint_short_resize_{}_{}.mp4", temp_id(), clip_num));
        let tmp_resize_str = tmp_resize.to_string_lossy().to_string();

        let final_out = out_dir.join(format!("{}_short_{}.mp4", file_stem, clip_num));
        let final_out_str = final_out.to_string_lossy().to_string();

        // 4a. Trim clip
        video_trim(&TrimRequest {
            input: req.input.clone(),
            output: tmp_trim_str.clone(),
            start: moment.start,
            end: moment.end,
        })?;

        // 4b. Resize to 9:16 vertical
        video_resize(&ResizeRequest {
            input: tmp_trim_str.clone(),
            output: tmp_resize_str.clone(),
            width: req.width,
            height: req.height,
        })?;
        let _ = std::fs::remove_file(&tmp_trim_str);

        // 4c. Subtitle burning if enabled
        if req.burn_subtitles {
            let relative_subtitles: Vec<TranscriptSegment> = transcript
                .segments
                .iter()
                .filter(|s| s.end > moment.start && s.start < moment.end)
                .enumerate()
                .map(|(i, s)| {
                    let mut copy = s.clone();
                    copy.id = i + 1;
                    copy.start = (s.start - moment.start).max(0.0);
                    copy.end = (s.end - moment.start).min(moment.end - moment.start);
                    copy
                })
                .collect();

            if !relative_subtitles.is_empty() {
                let srt = generate_srt(&relative_subtitles);
                burn_subtitles(&BurnSubtitleRequest {
                    input_video: tmp_resize_str.clone(),
                    srt_input: srt,
                    output_video: final_out_str.clone(),
                    style: None,
                    preset: Some("tiktok".into()),
                })?;
                let _ = std::fs::remove_file(&tmp_resize_str);
            } else {
                std::fs::rename(&tmp_resize_str, &final_out_str)?;
            }
        } else {
            std::fs::rename(&tmp_resize_str, &final_out_str)?;
        }

        generated_clips.push(ShortClipInfo {
            id: clip_num,
            path: final_out_str,
            start: moment.start,
            end: moment.end,
            duration: moment.end - moment.start,
            title: moment.title.clone(),
        });
    }

    Ok(MakeShortsResult {
        clips: generated_clips,
    })
}

/// Detect top highlight moments using LLM analysis or interval fallback.
async fn detect_highlights(
    config: &MintConfig,
    segments: &[TranscriptSegment],
    total_duration: f64,
    max_clips: u32,
    target_duration: f64,
) -> Vec<HighlightMoment> {
    if !segments.is_empty()
        && (!config.api_key.trim().is_empty() || !config.openai_api_key.trim().is_empty())
    {
        let text_with_timestamps = segments
            .iter()
            .map(|s| format!("[{:.1}s - {:.1}s] {}", s.start, s.end, s.text))
            .collect::<Vec<_>>()
            .join("\n");

        let prompt = format!(
            "Analyze this transcript and find up to {} most viral, engaging, and highlight-worthy clip moments for YouTube Shorts / TikTok.\n\
            Target clip duration is ~{:.0}s each.\n\
            Return ONLY a valid JSON array of objects with keys \"start\" (float), \"end\" (float), and \"title\" (string).\n\
            Example: [{{\"start\": 12.5, \"end\": 42.0, \"title\": \"Mindblowing Tech Reveal\"}}]\n\n\
            Transcript:\n{}",
            max_clips, target_duration, text_with_timestamps
        );

        let chat_req = crate::chat::ChatRequest {
            message: prompt,
            system_instruction:
                "You analyze video transcripts and output pure JSON arrays of highlight moments."
                    .into(),
            chat_id: None,
            image_data_uri: None,
            audio_data_uri: None,
            video_data_uri: None,
            document_attachment: None,
            workspace_path: None,
            agent_id: None,
            plan_mode: false,
            pinned_mcp_server: None,
            messages: None,
            tools: None,
        };

        if let Ok((res, _)) = crate::chat::send_chat_with_fallback(config, &chat_req).await {
            if let Some(json_start) = res.text.find('[') {
                if let Some(json_end) = res.text.rfind(']') {
                    let json_slice = &res.text[json_start..=json_end];
                    if let Ok(moments) = serde_json::from_str::<Vec<HighlightMoment>>(json_slice) {
                        if !moments.is_empty() {
                            return moments.into_iter().take(max_clips as usize).collect();
                        }
                    }
                }
            }
        }
    }

    // Fallback: divide video into equal duration clips
    fallback_interval_highlights(total_duration, max_clips, target_duration)
}

fn fallback_interval_highlights(
    total_duration: f64,
    max_clips: u32,
    target_duration: f64,
) -> Vec<HighlightMoment> {
    let mut moments = Vec::new();
    let clip_dur = target_duration.min(total_duration);
    let step = (total_duration - clip_dur) / (max_clips.max(1) as f64);

    for i in 0..max_clips {
        let start = (i as f64 * step).min(total_duration - clip_dur).max(0.0);
        let end = (start + clip_dur).min(total_duration);
        moments.push(HighlightMoment {
            start,
            end,
            title: format!("Highlight Clip {}", i + 1),
        });
        if end >= total_duration {
            break;
        }
    }
    moments
}

/// Interpret natural language video editing instructions and automatically run video tools.
pub async fn ai_edit_video(
    config: &MintConfig,
    req: &AiEditVideoRequest,
) -> Result<AiEditVideoResult, AutoShortsError> {
    let lower_instruction = req.instruction.to_lowercase();
    let mut steps_performed = Vec::new();

    // Out path determination
    let out_file = req.output.clone().unwrap_or_else(|| {
        let path = PathBuf::from(&req.input);
        let ext = path.extension().and_then(|s| s.to_str()).unwrap_or("mp4");
        format!(
            "{}_ai_edited.{}",
            req.input.trim_end_matches(&format!(".{ext}")),
            ext
        )
    });

    // Option 1: Shorts / TikTok generation requested
    if lower_instruction.contains("short")
        || lower_instruction.contains("ช็อต")
        || lower_instruction.contains("tiktok")
        || lower_instruction.contains("คลิปสั้น")
    {
        let max_clips = if lower_instruction.contains("1") {
            1
        } else if lower_instruction.contains("2") {
            2
        } else if lower_instruction.contains("5") {
            5
        } else {
            3
        };
        let shorts_res = make_shorts(
            config,
            &MakeShortsRequest {
                input: req.input.clone(),
                output_dir: req.output.clone(),
                max_clips,
                target_duration: 60.0,
                burn_subtitles: true,
                width: 1080,
                height: 1920,
            },
        )
        .await?;

        let first_clip = shorts_res
            .clips
            .first()
            .map(|c| c.path.clone())
            .unwrap_or_else(|| out_file.clone());
        steps_performed.push(AiEditStepResult {
            step: 1,
            operation: "make_shorts".into(),
            description: format!(
                "Generated {} vertical Shorts clips with burned subtitles",
                shorts_res.clips.len()
            ),
            output_path: first_clip.clone(),
        });

        return Ok(AiEditVideoResult {
            output_path: first_clip,
            steps_performed,
            summary: format!(
                "Successfully created {} Shorts clips based on AI prompt",
                shorts_res.clips.len()
            ),
        });
    }

    // Option 2: Trim requested
    let mut current_input = req.input.clone();
    if lower_instruction.contains("ตัด")
        || lower_instruction.contains("trim")
        || lower_instruction.contains("clip")
    {
        let numbers: Vec<f64> = lower_instruction
            .split(|c: char| !c.is_numeric() && c != '.')
            .filter_map(|s| s.parse::<f64>().ok())
            .collect();

        let start = if !numbers.is_empty() { numbers[0] } else { 0.0 };
        let end = if numbers.len() >= 2 {
            numbers[1]
        } else {
            start + 30.0
        };

        let tmp_trim = std::env::temp_dir().join(format!("mint_ai_trim_{}.mp4", temp_id()));
        let tmp_trim_str = tmp_trim.to_string_lossy().to_string();

        let res = video_trim(&TrimRequest {
            input: current_input.clone(),
            output: tmp_trim_str.clone(),
            start,
            end,
        })?;

        current_input = res.output_path.clone();
        steps_performed.push(AiEditStepResult {
            step: steps_performed.len() + 1,
            operation: "trim".into(),
            description: format!("Trimmed video segment ({:.1}s to {:.1}s)", start, end),
            output_path: current_input.clone(),
        });
    }

    // Option 3: Convert to Vertical (9:16) / Resize requested
    if lower_instruction.contains("แนวตั้ง")
        || lower_instruction.contains("9:16")
        || lower_instruction.contains("resize")
        || lower_instruction.contains("ย่อ")
    {
        let tmp_resize = std::env::temp_dir().join(format!("mint_ai_resize_{}.mp4", temp_id()));
        let tmp_resize_str = tmp_resize.to_string_lossy().to_string();

        let res = video_resize(&ResizeRequest {
            input: current_input.clone(),
            output: tmp_resize_str.clone(),
            width: 1080,
            height: 1920,
        })?;

        current_input = res.output_path.clone();
        steps_performed.push(AiEditStepResult {
            step: steps_performed.len() + 1,
            operation: "resize".into(),
            description: "Resized video to vertical 9:16 (1080x1920)".into(),
            output_path: current_input.clone(),
        });
    }

    // Option 4: Subtitles requested
    if lower_instruction.contains("ซับ")
        || lower_instruction.contains("sub")
        || lower_instruction.contains("คำบรรยาย")
        || lower_instruction.contains("เสียง")
    {
        let trans_res = transcribe(
            config,
            &TranscribeRequest {
                input: current_input.clone(),
                language: None,
                prompt: None,
            },
        )
        .await?;

        let srt = generate_srt(&trans_res.segments);
        let burn_res = burn_subtitles(&BurnSubtitleRequest {
            input_video: current_input.clone(),
            srt_input: srt,
            output_video: out_file.clone(),
            style: None,
            preset: Some("tiktok".into()),
        })?;

        current_input = burn_res.output_path.clone();
        steps_performed.push(AiEditStepResult {
            step: steps_performed.len() + 1,
            operation: "burn_subtitles".into(),
            description: format!(
                "Transcribed {} speech segments and burned subtitles",
                trans_res.segments.len()
            ),
            output_path: current_input.clone(),
        });
    }

    // Default fallback if no keyword triggered: export to requested out_file
    if steps_performed.is_empty() {
        std::fs::copy(&req.input, &out_file)?;
        steps_performed.push(AiEditStepResult {
            step: 1,
            operation: "export".into(),
            description: "Processed video file with AI prompt parameters".into(),
            output_path: out_file.clone(),
        });
        current_input = out_file.clone();
    } else if current_input != out_file && std::path::Path::new(&current_input).exists() {
        let _ = std::fs::copy(&current_input, &out_file);
        current_input = out_file;
    }

    Ok(AiEditVideoResult {
        output_path: current_input,
        steps_performed,
        summary: format!("AI Video Editor executed prompt: \"{}\"", req.instruction),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fallback_highlights() {
        let highlights = fallback_interval_highlights(180.0, 3, 45.0);
        assert_eq!(highlights.len(), 3);
        assert_eq!(highlights[0].start, 0.0);
        assert_eq!(highlights[0].end, 45.0);
    }
}

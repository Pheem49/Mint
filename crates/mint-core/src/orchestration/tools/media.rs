use std::path::Path;

use super::super::*;

/// Handles the subset of `execute_tool` actions related to media.
/// Only called for actions `execute_tool` has already routed here, so the
/// fallback arm is unreachable in practice.
pub(in crate::orchestration) async fn execute(
    action: &str,
    input: &AgentInput,
    _root: &Path,
    config: &MintConfig,
    _chat_id: &str,
    _approve_cb: &mut (dyn FnMut(&AgentApproval) -> Result<ApprovalOutcome, String> + Send),
) -> Result<String, OrchestrationError> {
    match action {
        "video_trim" | "video.trim" => {
            let input_path = required(&input.input, "input")?;
            let output_path = required(&input.output, "output")?;
            let req = crate::video_edit::TrimRequest {
                input: input_path.to_string(),
                output: output_path.to_string(),
                start: input.start.unwrap_or(0.0),
                end: input.end.unwrap_or(0.0),
            };
            let res = crate::video_edit::video_trim(&req)
                .map_err(|e| OrchestrationError::Agent(e.to_string()))?;
            Ok(serde_json::to_string(&res).unwrap_or_default())
        }
        "video_remove_silence" | "video.remove_silence" => {
            let input_path = required(&input.input, "input")?;
            let output_path = required(&input.output, "output")?;
            let req = crate::video_edit::RemoveSilenceRequest {
                input: input_path.to_string(),
                output: output_path.to_string(),
                threshold_db: input.threshold_db.unwrap_or(-30.0),
                min_silence_secs: input.min_silence_secs.unwrap_or(0.5),
            };
            let res = crate::video_edit::video_remove_silence(&req)
                .map_err(|e| OrchestrationError::Agent(e.to_string()))?;
            Ok(serde_json::to_string(&res).unwrap_or_default())
        }
        "video_resize" => {
            let input_path = required(&input.input, "input")?;
            let output_path = required(&input.output, "output")?;
            let req = crate::video_edit::ResizeRequest {
                input: input_path.to_string(),
                output: output_path.to_string(),
                width: input.width.unwrap_or(1920),
                height: input.height.unwrap_or(1080),
            };
            let res = crate::video_edit::video_resize(&req)
                .map_err(|e| OrchestrationError::Agent(e.to_string()))?;
            Ok(serde_json::to_string(&res).unwrap_or_default())
        }
        "video_merge" => {
            let output_path = required(&input.output, "output")?;
            let req = crate::video_edit::MergeRequest {
                inputs: if input.inputs.is_empty() {
                    input.commands.clone()
                } else {
                    input.inputs.clone()
                },
                output: output_path.to_string(),
            };
            let res = crate::video_edit::video_merge(&req)
                .map_err(|e| OrchestrationError::Agent(e.to_string()))?;
            Ok(serde_json::to_string(&res).unwrap_or_default())
        }
        "video_export" | "video.export" => {
            let input_path = required(&input.input, "input")?;
            let output_path = required(&input.output, "output")?;
            let req = crate::video_edit::ExportRequest {
                input: input_path.to_string(),
                output: output_path.to_string(),
                resolution: input.preset.clone(),
                fps: None,
                codec: None,
                crf: None,
            };
            let res = crate::video_edit::video_export(&req)
                .map_err(|e| OrchestrationError::Agent(e.to_string()))?;
            Ok(serde_json::to_string(&res).unwrap_or_default())
        }
        "video_extract_audio" => {
            let input_path = required(&input.input, "input")?;
            let output_path = required(&input.output, "output")?;
            let req = crate::video_edit::ExtractAudioRequest {
                input: input_path.to_string(),
                output: output_path.to_string(),
            };
            let out = crate::video_edit::video_extract_audio(&req)
                .map_err(|e| OrchestrationError::Agent(e.to_string()))?;
            Ok(format!("Audio extracted to {}", out.output_path))
        }
        "video_filmstrip" | "video.filmstrip" => {
            let input_path = required(&input.input, "input")?;
            let output_path = required(&input.output, "output")?;
            let req = crate::video_edit::FilmstripRequest {
                input: input_path.to_string(),
                output: output_path.to_string(),
                frame_count: input.frame_count.unwrap_or(12),
                columns: input.columns.unwrap_or(4),
                thumb_width: input
                    .width
                    .filter(|w| *w > 0)
                    .map(|w| w as u32)
                    .unwrap_or(320),
            };
            let res = crate::video_edit::video_filmstrip(&req)
                .map_err(|e| OrchestrationError::Agent(e.to_string()))?;
            let bytes = std::fs::read(&res.output_path).map_err(|e| {
                OrchestrationError::Agent(format!("failed to read generated filmstrip: {e}"))
            })?;
            Ok(format!(
                "data:image/png;base64,{}",
                BASE64_STANDARD.encode(bytes)
            ))
        }
        "video_waveform" | "video.waveform" => {
            let input_path = required(&input.input, "input")?;
            let output_path = required(&input.output, "output")?;
            let req = crate::video_edit::WaveformRequest {
                input: input_path.to_string(),
                output: output_path.to_string(),
                width: input
                    .width
                    .filter(|w| *w > 0)
                    .map(|w| w as u32)
                    .unwrap_or(1280),
                height: input
                    .height
                    .filter(|h| *h > 0)
                    .map(|h| h as u32)
                    .unwrap_or(240),
            };
            let res = crate::video_edit::video_waveform(&req)
                .map_err(|e| OrchestrationError::Agent(e.to_string()))?;
            let bytes = std::fs::read(&res.output_path).map_err(|e| {
                OrchestrationError::Agent(format!("failed to read generated waveform: {e}"))
            })?;
            Ok(format!(
                "data:image/png;base64,{}",
                BASE64_STANDARD.encode(bytes)
            ))
        }
        "speech_transcribe" | "subtitle_generate" | "subtitle.generate" => {
            let input_path = required(&input.input, "input")?;
            let req = crate::speech::TranscribeRequest {
                input: input_path.to_string(),
                language: input.language.clone(),
                prompt: None,
            };
            let res = crate::speech::transcribe(config, &req)
                .await
                .map_err(|e| OrchestrationError::Agent(e.to_string()))?;
            Ok(serde_json::to_string(&res).unwrap_or_default())
        }
        "subtitle_translate" | "subtitle.translate" => {
            let srt = input.srt_content.as_deref().unwrap_or_default();
            let target = input.target_language.as_deref().unwrap_or("th");
            let req = crate::subtitle::TranslateSubtitleRequest {
                srt_content: srt.to_string(),
                target_language: target.to_string(),
            };
            let translated = crate::subtitle::translate_subtitles(config, &req)
                .await
                .map_err(|e| OrchestrationError::Agent(e.to_string()))?;
            Ok(translated)
        }
        "subtitle_burn" => {
            let input_video = required(&input.input, "input")?;
            let output_video = required(&input.output, "output")?;
            let srt_input = input.srt_content.as_deref().unwrap_or_default();
            let req = crate::subtitle::BurnSubtitleRequest {
                input_video: input_video.to_string(),
                srt_input: srt_input.to_string(),
                output_video: output_video.to_string(),
                style: None,
                preset: input.preset.clone(),
            };
            let res = crate::subtitle::burn_subtitles(&req)
                .map_err(|e| OrchestrationError::Agent(e.to_string()))?;
            Ok(serde_json::to_string(&res).unwrap_or_default())
        }
        "timeline_reorder" | "timeline.reorder" => {
            let output_path = required(&input.output, "output")?;
            let req = crate::video_edit::ReorderClipsRequest {
                inputs: input.inputs.clone(),
                order: input.order.clone(),
                output: output_path.to_string(),
            };
            let res = crate::video_edit::timeline_reorder(&req)
                .map_err(|e| OrchestrationError::Agent(e.to_string()))?;
            Ok(serde_json::to_string(&res).unwrap_or_default())
        }
        "effect_zoom_on_speaker" | "effect.zoom_on_speaker" => {
            let input_path = required(&input.input, "input")?;
            let output_path = required(&input.output, "output")?;
            let req = crate::video_edit::ZoomSpeakerRequest {
                input: input_path.to_string(),
                output: output_path.to_string(),
                zoom_factor: input.zoom_factor.unwrap_or(1.25),
            };
            let res = crate::video_edit::effect_zoom_on_speaker(&req)
                .map_err(|e| OrchestrationError::Agent(e.to_string()))?;
            Ok(serde_json::to_string(&res).unwrap_or_default())
        }
        "audio_duck_music" | "audio.duck_music" => {
            let video_in = input.video_input.as_deref().unwrap_or(&input.input);
            let music_in = input.music_input.as_deref().unwrap_or("");
            let output_path = required(&input.output, "output")?;
            let req = crate::video_edit::DuckMusicRequest {
                video_input: video_in.to_string(),
                music_input: music_in.to_string(),
                output: output_path.to_string(),
                music_volume: input.music_volume.unwrap_or(0.2),
            };
            let res = crate::video_edit::audio_duck_music(&req)
                .map_err(|e| OrchestrationError::Agent(e.to_string()))?;
            Ok(serde_json::to_string(&res).unwrap_or_default())
        }
        "make_shorts" | "video.make_shorts" => {
            let input_path = required(&input.input, "input")?;
            let req = crate::auto_shorts::MakeShortsRequest {
                input: input_path.to_string(),
                output_dir: if input.output.is_empty() {
                    None
                } else {
                    Some(input.output.clone())
                },
                max_clips: input.max_clips.unwrap_or(3),
                target_duration: input.target_duration.unwrap_or(60.0),
                burn_subtitles: true,
                width: input.width.unwrap_or(1080),
                height: input.height.unwrap_or(1920),
            };
            let res = crate::auto_shorts::make_shorts(config, &req)
                .await
                .map_err(|e| OrchestrationError::Agent(e.to_string()))?;
            Ok(serde_json::to_string(&res).unwrap_or_default())
        }
        "generate_image" | "image_studio.generate" | "image_generate" => {
            let prompt_text = if !input.prompt.trim().is_empty() {
                input.prompt.trim()
            } else if !input.query.trim().is_empty() {
                input.query.trim()
            } else {
                required(&input.text, "prompt")?
            };
            let req = crate::image_gen::ImageGenRequest {
                prompt: prompt_text.to_string(),
                aspect_ratio: if input.aspect_ratio.is_empty() {
                    Some("1:1".to_string())
                } else {
                    Some(input.aspect_ratio.clone())
                },
                provider: if input.provider.is_empty() {
                    None
                } else {
                    Some(input.provider.clone())
                },
                num_images: Some(1),
                ..Default::default()
            };
            let res = crate::image_gen::generate_images(config, &req)
                .await
                .map_err(|e| OrchestrationError::Agent(e.to_string()))?;
            let data_uris: Vec<String> = res.images.iter().map(|i| i.data_uri.clone()).collect();
            if let Ok(saved) = crate::pictures::save_chat_images(
                data_uris,
                Some(res.provider.clone()),
                Some(prompt_text.to_string()),
            ) {
                if !saved.is_empty() {
                    let saved_path = saved[0].path.display().to_string();
                    let json_payload = serde_json::json!({
                        "prompt": prompt_text,
                        "model": res.model,
                        "provider": res.provider,
                        "images": saved.iter()
                            .map(|s| serde_json::json!({ "url": format!("/api/pictures/{}", s.filename) }))
                            .collect::<Vec<_>>(),
                    });
                    let json_str = serde_json::to_string_pretty(&json_payload).unwrap_or_default();
                    let data = format!(
                        "Image generated successfully with model `{}` ({}). Saved to: {}\n\n```image_gen_json\n{}\n```\n\nNote: Image generation succeeded. In your finish summary, you MUST include the exact ```image_gen_json ... ``` code block from above in your response so the user sees the generated image card.",
                        res.model, res.provider, saved_path, json_str
                    );
                    return Ok(data);
                }
            }
            if let Some(first) = res.images.first() {
                let json_payload = serde_json::json!({
                    "prompt": prompt_text,
                    "model": res.model,
                    "provider": res.provider,
                    "images": [{ "url": first.data_uri }],
                });
                let json_str = serde_json::to_string_pretty(&json_payload).unwrap_or_default();
                let data = format!(
                    "Image generated successfully with model `{}` ({}).\n\n```image_gen_json\n{}\n```\n\nNote: Image generation succeeded. In your finish summary, you MUST include the exact ```image_gen_json ... ``` code block from above in your response so the user sees the generated image card.",
                    res.model, res.provider, json_str
                );
                Ok(data)
            } else {
                Ok("No image returned from provider".to_string())
            }
        }
        "generate_video" | "veo.generate" | "video_generate" => {
            let prompt_text = if !input.prompt.trim().is_empty() {
                input.prompt.trim()
            } else if !input.query.trim().is_empty() {
                input.query.trim()
            } else {
                required(&input.text, "prompt")?
            };
            let req = crate::video_gen::VideoGenRequest {
                prompt: prompt_text.to_string(),
                negative_prompt: None,
                aspect_ratio: if input.aspect_ratio.is_empty() {
                    "16:9".to_string()
                } else {
                    input.aspect_ratio.clone()
                },
                duration: input.duration.unwrap_or(5.0) as u32,
                model: None,
                provider: if input.provider.is_empty() {
                    "veo".to_string()
                } else {
                    input.provider.clone()
                },
            };
            let res = crate::video_gen::generate_video(config, &req)
                .await
                .map_err(|e| OrchestrationError::Agent(e.to_string()))?;
            if let Some(first) = res.videos.first() {
                let vid_md = format!(
                    "<video controls src=\"{}\" width=\"100%\" style=\"max-height:400px; border-radius:8px;\"></video>\n\n✓ Video generated successfully with Veo `{}` ({})",
                    first.path.to_string_lossy(),
                    res.model,
                    res.provider
                );
                Ok(vid_md)
            } else {
                Ok("No video returned from provider".to_string())
            }
        }
        _ => unreachable!(
            "execute_tool routed an unhandled action into tools::media::execute: {action}"
        ),
    }
}

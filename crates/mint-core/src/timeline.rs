use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::video_edit::{
    ExportRequest, MergeRequest, ResizeRequest, TrimRequest, VideoEditError,
    check_ffmpeg, run_ffmpeg_args, video_export, video_merge, video_resize, video_trim,
};

// ── Error ──────────────────────────────────────────────────────────────────

#[derive(Debug, Error)]
pub enum TimelineError {
    #[error("video edit error: {0}")]
    VideoEdit(#[from] VideoEditError),
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("render error: {0}")]
    Render(String),
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
}

// ── Data model ─────────────────────────────────────────────────────────────

/// A single clip in the timeline.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TimelineClip {
    /// Absolute path to the source video file.
    pub source: String,
    /// Trim start in seconds (0 = beginning of clip).
    #[serde(default)]
    pub trim_start: f64,
    /// Trim end in seconds (0 = use full duration).
    #[serde(default)]
    pub trim_end: f64,
    /// Playback order index.
    #[serde(default)]
    pub order: u32,
    /// Optional scale/crop to apply to this clip.
    #[serde(default)]
    pub scale: Option<ScaleEffect>,
}

/// Scale/crop effect applied to a clip.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScaleEffect {
    pub width: i32,
    pub height: i32,
}

/// A subtitle entry in the timeline.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TimelineSubtitle {
    pub start: f64,
    pub end: f64,
    pub text: String,
}

/// An overlay / visual effect applied to the timeline.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum TimelineEffect {
    /// Zoom in on a region at a specific timestamp.
    Zoom {
        #[serde(default)]
        at: f64,
        scale: f64,
    },
    /// Blur a rectangular region.
    Blur {
        x: u32,
        y: u32,
        width: u32,
        height: u32,
        #[serde(default)]
        at: f64,
    },
    /// Cross-dissolve or cut transition between clips.
    Transition {
        kind: String,
        duration_secs: f64,
    },
}

/// Audio configuration for the timeline.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TimelineAudio {
    /// Optional background music track path.
    #[serde(default)]
    pub music: Option<String>,
    /// If true, duck (lower) music volume when speech is detected.
    #[serde(default)]
    pub duck: bool,
    /// Music volume 0.0–1.0 (default 0.5).
    #[serde(default = "default_music_volume")]
    pub music_volume: f64,
    /// Ducked music volume when speech is present (default 0.1).
    #[serde(default = "default_duck_volume")]
    pub duck_volume: f64,
}

fn default_music_volume() -> f64 {
    0.5
}
fn default_duck_volume() -> f64 {
    0.1
}

/// Output encoding settings.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TimelineOutput {
    /// Output file path.
    pub path: String,
    /// e.g. "1920x1080", "1280x720" — empty = keep source resolution.
    #[serde(default)]
    pub resolution: Option<String>,
    /// Frames per second — None = keep source fps.
    #[serde(default)]
    pub fps: Option<u32>,
    /// Video codec (default: "libx264").
    #[serde(default)]
    pub codec: Option<String>,
    /// CRF quality (default: 23).
    #[serde(default)]
    pub crf: Option<u32>,
}

impl Default for TimelineOutput {
    fn default() -> Self {
        Self {
            path: "output.mp4".to_string(),
            resolution: None,
            fps: None,
            codec: None,
            crf: None,
        }
    }
}

/// The full JSON timeline — the central data structure for all editing operations.
///
/// Every editing capability produces or modifies a Timeline, which is then
/// rendered to a final video by `render_timeline()`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Timeline {
    /// Ordered list of video clips. Sorted by `order` before rendering.
    #[serde(default)]
    pub clips: Vec<TimelineClip>,
    /// Visual effects applied to the composition.
    #[serde(default)]
    pub effects: Vec<TimelineEffect>,
    /// Subtitle entries to burn into the video.
    #[serde(default)]
    pub subtitles: Vec<TimelineSubtitle>,
    /// Audio / music configuration.
    #[serde(default)]
    pub audio: TimelineAudio,
    /// Output encoding settings.
    pub output: TimelineOutput,
}

// ── Request / Response types ───────────────────────────────────────────────

/// Request body for `/api/video/render-timeline`.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RenderTimelineRequest {
    pub timeline: Timeline,
}

/// Result of a timeline render.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RenderTimelineResult {
    pub output_path: String,
    pub clips_rendered: usize,
    pub duration: Option<f64>,
    pub size_bytes: Option<u64>,
}

// ── render_timeline ────────────────────────────────────────────────────────

/// Render a `Timeline` to a final video file.
///
/// Pipeline:
/// 1. Sort clips by order.
/// 2. For each clip: trim + scale if specified → write temp segment.
/// 3. Merge all temp segments.
/// 4. If subtitles present, burn them using FFmpeg `subtitles` filter.
/// 5. If music is specified, mix audio.
/// 6. Re-encode to final output settings.
pub fn render_timeline(timeline: &Timeline) -> Result<RenderTimelineResult, TimelineError> {
    check_ffmpeg()?;

    if timeline.clips.is_empty() {
        return Err(TimelineError::Render("timeline has no clips".into()));
    }

    let tmp_dir = std::env::temp_dir().join("mint_timeline_render");
    let _ = std::fs::create_dir_all(&tmp_dir);

    // Step 1: Sort clips by order
    let mut sorted_clips = timeline.clips.clone();
    sorted_clips.sort_by_key(|c| c.order);

    // Step 2: Process each clip (trim + scale)
    let mut segment_paths: Vec<String> = Vec::new();
    for (i, clip) in sorted_clips.iter().enumerate() {
        let seg_path = tmp_dir.join(format!("clip_{i:04}.mp4"));
        let seg_str = seg_path.to_string_lossy().to_string();

        // Determine the effective trim range
        let trim_end = if clip.trim_end > 0.0 {
            clip.trim_end
        } else {
            // Use full duration — get it via video_load
            crate::video_edit::video_load(&clip.source)
                .map(|info| info.duration)
                .unwrap_or(f64::MAX)
        };

        if clip.trim_start > 0.0 || trim_end < f64::MAX {
            let trim_req = TrimRequest {
                input: clip.source.clone(),
                output: seg_str.clone(),
                start: clip.trim_start,
                end: trim_end,
            };
            video_trim(&trim_req)?;
        } else {
            // No trim needed: symlink or copy
            std::fs::copy(&clip.source, &seg_str)?;
        }

        // Apply scale if specified
        if let Some(scale) = &clip.scale {
            let scaled_path = tmp_dir.join(format!("clip_{i:04}_scaled.mp4"));
            let scaled_str = scaled_path.to_string_lossy().to_string();
            let resize_req = ResizeRequest {
                input: seg_str.clone(),
                output: scaled_str.clone(),
                width: scale.width,
                height: scale.height,
            };
            video_resize(&resize_req)?;
            let _ = std::fs::remove_file(&seg_str);
            segment_paths.push(scaled_str);
        } else {
            segment_paths.push(seg_str);
        }
    }

    // Step 3: Merge segments
    let merged_path = tmp_dir.join("merged.mp4");
    let merged_str = merged_path.to_string_lossy().to_string();

    if segment_paths.len() == 1 {
        std::fs::copy(&segment_paths[0], &merged_str)?;
    } else {
        let merge_req = MergeRequest {
            inputs: segment_paths.clone(),
            output: merged_str.clone(),
        };
        video_merge(&merge_req)?;
    }

    // Step 4: Burn subtitles (if any)
    let after_subs_path = if !timeline.subtitles.is_empty() {
        let srt_path = tmp_dir.join("subtitles.srt");
        let srt_str = srt_path.to_string_lossy().to_string();
        write_srt_file(&timeline.subtitles, &srt_str)?;

        let subbed_path = tmp_dir.join("with_subs.mp4");
        let subbed_str = subbed_path.to_string_lossy().to_string();

        // Escape path for FFmpeg filter syntax
        let escaped = srt_str.replace('\\', "\\\\").replace(':', "\\:");
        let vf = format!("subtitles='{}'", escaped);

        run_ffmpeg_args(&["-y", "-i", &merged_str, "-vf", &vf, "-c:a", "copy", &subbed_str])?;
        subbed_str
    } else {
        merged_str.clone()
    };

    // Step 5: Mix music (if provided)
    let after_audio_path = if let Some(music_path) = &timeline.audio.music {
        let mixed_path = tmp_dir.join("with_music.mp4");
        let mixed_str = mixed_path.to_string_lossy().to_string();
        let vol = timeline.audio.music_volume;
        let duck = timeline.audio.duck_volume;

        // Simple approach: mix video audio with music at given volume
        // If duck is enabled, use a simplified constant volume reduction
        let music_vol = if timeline.audio.duck { duck } else { vol };
        let audio_filter = format!(
            "[0:a]volume=1.0[va];[1:a]volume={}[ma];[va][ma]amix=inputs=2:duration=shortest[outa]",
            music_vol
        );

        run_ffmpeg_args(&[
            "-y",
            "-i",
            &after_subs_path,
            "-i",
            music_path,
            "-filter_complex",
            &audio_filter,
            "-map",
            "0:v",
            "-map",
            "[outa]",
            "-c:v",
            "copy",
            "-c:a",
            "aac",
            "-b:a",
            "192k",
            "-shortest",
            &mixed_str,
        ])?;
        mixed_str
    } else {
        after_subs_path
    };

    // Step 6: Final export with output settings
    let export_req = ExportRequest {
        input: after_audio_path,
        output: timeline.output.path.clone(),
        resolution: timeline.output.resolution.clone(),
        fps: timeline.output.fps,
        codec: timeline.output.codec.clone(),
        crf: timeline.output.crf,
    };
    let export_result = video_export(&export_req)?;

    // Cleanup temp directory
    for seg in &segment_paths {
        let _ = std::fs::remove_file(seg);
    }
    let _ = std::fs::remove_dir_all(&tmp_dir);

    let info = crate::video_edit::video_load(&export_result.output_path).ok();
    Ok(RenderTimelineResult {
        output_path: export_result.output_path,
        clips_rendered: sorted_clips.len(),
        duration: info.as_ref().map(|v| v.duration),
        size_bytes: info.as_ref().map(|v| v.size_bytes),
    })
}

/// Write a Vec<TimelineSubtitle> to an SRT file on disk.
fn write_srt_file(subtitles: &[TimelineSubtitle], path: &str) -> Result<(), TimelineError> {
    let mut srt = String::new();
    for (i, sub) in subtitles.iter().enumerate() {
        srt.push_str(&format!(
            "{}\n{} --> {}\n{}\n\n",
            i + 1,
            secs_to_srt_time(sub.start),
            secs_to_srt_time(sub.end),
            sub.text
        ));
    }
    std::fs::write(path, srt)?;
    Ok(())
}

fn secs_to_srt_time(secs: f64) -> String {
    let h = (secs / 3600.0) as u32;
    let m = ((secs % 3600.0) / 60.0) as u32;
    let s = (secs % 60.0) as u32;
    let ms = ((secs % 1.0) * 1000.0) as u32;
    format!("{:02}:{:02}:{:02},{:03}", h, m, s, ms)
}

/// Serialize a Timeline to a pretty-printed JSON string.
pub fn timeline_to_json(timeline: &Timeline) -> Result<String, TimelineError> {
    Ok(serde_json::to_string_pretty(timeline)?)
}

/// Parse a JSON string into a Timeline.
pub fn timeline_from_json(json: &str) -> Result<Timeline, TimelineError> {
    Ok(serde_json::from_str(json)?)
}

// ── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_secs_to_srt_time() {
        assert_eq!(secs_to_srt_time(0.0), "00:00:00,000");
        assert_eq!(secs_to_srt_time(3661.5), "01:01:01,500");
        assert_eq!(secs_to_srt_time(90.25), "00:01:30,250");
    }

    #[test]
    fn test_timeline_roundtrip() {
        let timeline = Timeline {
            clips: vec![TimelineClip {
                source: "/tmp/test.mp4".into(),
                trim_start: 0.0,
                trim_end: 30.0,
                order: 0,
                scale: None,
            }],
            effects: vec![],
            subtitles: vec![TimelineSubtitle {
                start: 0.0,
                end: 3.0,
                text: "Hello World".into(),
            }],
            audio: TimelineAudio::default(),
            output: TimelineOutput {
                path: "/tmp/out.mp4".into(),
                ..Default::default()
            },
        };

        let json = timeline_to_json(&timeline).unwrap();
        let parsed = timeline_from_json(&json).unwrap();
        assert_eq!(parsed.clips.len(), 1);
        assert_eq!(parsed.subtitles.len(), 1);
        assert_eq!(parsed.subtitles[0].text, "Hello World");
    }
}

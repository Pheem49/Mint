use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::process::Command;
use thiserror::Error;

// ── Error ──────────────────────────────────────────────────────────────────

#[derive(Debug, Error)]
pub enum VideoEditError {
    #[error("ffmpeg not found — please install ffmpeg (https://ffmpeg.org/download.html)")]
    FfmpegNotFound,
    #[error("ffprobe not found — please install ffmpeg with ffprobe")]
    FfprobeNotFound,
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("ffmpeg/ffprobe error: {0}")]
    Process(String),
    #[error("invalid video parameters: {0}")]
    InvalidParams(String),
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
}

// ── Data types ─────────────────────────────────────────────────────────────

/// Metadata about a video file returned by `video_load`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VideoInfo {
    pub path: String,
    pub duration: f64,
    pub fps: f64,
    pub width: u32,
    pub height: u32,
    pub has_audio: bool,
    pub audio_streams: u32,
    pub size_bytes: u64,
    pub format: String,
}

/// Result of a single video editing operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VideoEditResult {
    pub output_path: String,
    pub duration: Option<f64>,
    pub size_bytes: Option<u64>,
}

/// Request body for the `/api/video/trim` route.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TrimRequest {
    pub input: String,
    pub output: String,
    pub start: f64,
    pub end: f64,
}

/// Request body for `/api/video/crop`.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CropRequest {
    pub input: String,
    pub output: String,
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}

/// Request body for `/api/video/resize`.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResizeRequest {
    pub input: String,
    pub output: String,
    /// Target width in pixels (-1 to preserve aspect ratio).
    pub width: i32,
    /// Target height in pixels (-1 to preserve aspect ratio).
    pub height: i32,
}

/// Request body for `/api/video/merge`.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MergeRequest {
    pub inputs: Vec<String>,
    pub output: String,
}

/// Request body for `/api/video/extract-audio`.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExtractAudioRequest {
    pub input: String,
    pub output: String,
}

/// Request body for `/api/video/remove-silence`.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoveSilenceRequest {
    pub input: String,
    pub output: String,
    /// dB threshold below which audio is considered silent (default: -30).
    #[serde(default = "default_silence_threshold")]
    pub threshold_db: f64,
    /// Minimum silence duration in seconds to remove (default: 0.5).
    #[serde(default = "default_min_silence")]
    pub min_silence_secs: f64,
}

fn default_silence_threshold() -> f64 {
    -30.0
}
fn default_min_silence() -> f64 {
    0.5
}

/// Export options for the final render.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportRequest {
    pub input: String,
    pub output: String,
    /// e.g. "1920x1080", "1280x720", "3840x2160" — leave empty to keep source resolution.
    #[serde(default)]
    pub resolution: Option<String>,
    /// Output frames per second (e.g. 30, 60). None keeps source fps.
    #[serde(default)]
    pub fps: Option<u32>,
    /// Video codec: "libx264" (default), "libx265", "libvpx-vp9"
    #[serde(default)]
    pub codec: Option<String>,
    /// CRF quality 0–51 (lower = better quality, default 23)
    #[serde(default)]
    pub crf: Option<u32>,
}

// ── Helpers ────────────────────────────────────────────────────────────────

/// Checks whether `ffmpeg` is available on PATH.
pub fn check_ffmpeg() -> Result<(), VideoEditError> {
    Command::new("ffmpeg")
        .arg("-version")
        .output()
        .map_err(|_| VideoEditError::FfmpegNotFound)?;
    Ok(())
}

/// Checks whether `ffprobe` is available on PATH.
pub fn check_ffprobe() -> Result<(), VideoEditError> {
    Command::new("ffprobe")
        .arg("-version")
        .output()
        .map_err(|_| VideoEditError::FfprobeNotFound)?;
    Ok(())
}

/// Runs an ffmpeg command synchronously and returns stderr on non-zero exit.
/// Public alias for use by sibling modules (e.g. `timeline`).
pub fn run_ffmpeg_args(args: &[&str]) -> Result<(), VideoEditError> {
    run_ffmpeg(args)
}

/// Runs an ffmpeg command synchronously and returns stderr on non-zero exit.
fn run_ffmpeg(args: &[&str]) -> Result<(), VideoEditError> {
    let output = Command::new("ffmpeg")
        .args(args)
        .output()
        .map_err(|_| VideoEditError::FfmpegNotFound)?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        return Err(VideoEditError::Process(stderr));
    }
    Ok(())
}

/// Returns the file size in bytes (best-effort).
fn file_size(path: &str) -> Option<u64> {
    std::fs::metadata(path).ok().map(|m| m.len())
}

// ── video_load ─────────────────────────────────────────────────────────────

/// Inspect a video file using `ffprobe` and return metadata.
pub fn video_load(path: &str) -> Result<VideoInfo, VideoEditError> {
    check_ffprobe()?;

    let output = Command::new("ffprobe")
        .args([
            "-v",
            "quiet",
            "-print_format",
            "json",
            "-show_streams",
            "-show_format",
            path,
        ])
        .output()
        .map_err(|_| VideoEditError::FfprobeNotFound)?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        return Err(VideoEditError::Process(stderr));
    }

    let raw: Value = serde_json::from_slice(&output.stdout)?;
    let streams = raw["streams"].as_array().cloned().unwrap_or_default();

    let video_stream = streams.iter().find(|s| s["codec_type"] == "video");
    let audio_stream_count = streams
        .iter()
        .filter(|s| s["codec_type"] == "audio")
        .count() as u32;

    let width = video_stream
        .and_then(|s| s["width"].as_u64())
        .unwrap_or(0) as u32;
    let height = video_stream
        .and_then(|s| s["height"].as_u64())
        .unwrap_or(0) as u32;

    let fps = video_stream
        .and_then(|s| s["r_frame_rate"].as_str())
        .and_then(parse_fraction)
        .unwrap_or(0.0);

    let duration = raw["format"]["duration"]
        .as_str()
        .and_then(|d| d.parse::<f64>().ok())
        .or_else(|| {
            video_stream
                .and_then(|s| s["duration"].as_str())
                .and_then(|d| d.parse().ok())
        })
        .unwrap_or(0.0);

    let format = raw["format"]["format_name"]
        .as_str()
        .unwrap_or("unknown")
        .to_string();

    let size_bytes = raw["format"]["size"]
        .as_str()
        .and_then(|s| s.parse::<u64>().ok())
        .or_else(|| std::fs::metadata(path).ok().map(|m| m.len()))
        .unwrap_or(0);

    Ok(VideoInfo {
        path: path.to_string(),
        duration,
        fps,
        width,
        height,
        has_audio: audio_stream_count > 0,
        audio_streams: audio_stream_count,
        size_bytes,
        format,
    })
}

fn parse_fraction(s: &str) -> Option<f64> {
    let parts: Vec<&str> = s.split('/').collect();
    if parts.len() == 2 {
        let num = parts[0].parse::<f64>().ok()?;
        let den = parts[1].parse::<f64>().ok()?;
        if den != 0.0 {
            return Some(num / den);
        }
    }
    s.parse::<f64>().ok()
}

// ── video_trim ─────────────────────────────────────────────────────────────

/// Trim a video between `start` and `end` seconds.
pub fn video_trim(req: &TrimRequest) -> Result<VideoEditResult, VideoEditError> {
    check_ffmpeg()?;
    if req.start >= req.end {
        return Err(VideoEditError::InvalidParams(
            "start must be less than end".into(),
        ));
    }
    let start_str = format!("{}", req.start);
    let duration_str = format!("{}", req.end - req.start);

    run_ffmpeg(&[
        "-y",
        "-ss",
        &start_str,
        "-i",
        &req.input,
        "-t",
        &duration_str,
        "-c",
        "copy",
        &req.output,
    ])?;

    Ok(VideoEditResult {
        output_path: req.output.clone(),
        duration: Some(req.end - req.start),
        size_bytes: file_size(&req.output),
    })
}

// ── video_crop ─────────────────────────────────────────────────────────────

/// Crop a video to the given rectangle.
pub fn video_crop(req: &CropRequest) -> Result<VideoEditResult, VideoEditError> {
    check_ffmpeg()?;
    let filter = format!("crop={}:{}:{}:{}", req.width, req.height, req.x, req.y);
    run_ffmpeg(&[
        "-y",
        "-i",
        &req.input,
        "-vf",
        &filter,
        "-c:a",
        "copy",
        &req.output,
    ])?;
    let info = video_load(&req.output);
    Ok(VideoEditResult {
        output_path: req.output.clone(),
        duration: info.as_ref().ok().map(|v| v.duration),
        size_bytes: file_size(&req.output),
    })
}

// ── video_resize ───────────────────────────────────────────────────────────

/// Resize / scale a video. Use -1 for width or height to preserve aspect ratio.
/// Width and height are rounded to even numbers (required by most codecs).
pub fn video_resize(req: &ResizeRequest) -> Result<VideoEditResult, VideoEditError> {
    check_ffmpeg()?;
    let w = if req.width == -1 {
        "-2".to_string()
    } else {
        let even = if req.width % 2 != 0 {
            req.width + 1
        } else {
            req.width
        };
        even.to_string()
    };
    let h = if req.height == -1 {
        "-2".to_string()
    } else {
        let even = if req.height % 2 != 0 {
            req.height + 1
        } else {
            req.height
        };
        even.to_string()
    };
    let filter = format!("scale={}:{}", w, h);
    run_ffmpeg(&[
        "-y",
        "-i",
        &req.input,
        "-vf",
        &filter,
        "-c:a",
        "copy",
        &req.output,
    ])?;
    let info = video_load(&req.output);
    Ok(VideoEditResult {
        output_path: req.output.clone(),
        duration: info.as_ref().ok().map(|v| v.duration),
        size_bytes: file_size(&req.output),
    })
}

// ── video_merge ────────────────────────────────────────────────────────────

/// Concatenate multiple video clips into one output file.
pub fn video_merge(req: &MergeRequest) -> Result<VideoEditResult, VideoEditError> {
    check_ffmpeg()?;
    if req.inputs.is_empty() {
        return Err(VideoEditError::InvalidParams(
            "at least one input is required".into(),
        ));
    }

    // Write a temporary concat list file
    let list_path = {
        let mut p = std::env::temp_dir();
        p.push("mint_video_concat_list.txt");
        p
    };
    let list_content = req
        .inputs
        .iter()
        .map(|p| format!("file '{}'\n", p.replace('\'', "'\\''")))
        .collect::<String>();
    std::fs::write(&list_path, list_content)?;

    let list_str = list_path.to_string_lossy().to_string();
    run_ffmpeg(&[
        "-y",
        "-f",
        "concat",
        "-safe",
        "0",
        "-i",
        &list_str,
        "-c",
        "copy",
        &req.output,
    ])?;
    let _ = std::fs::remove_file(&list_path);

    let info = video_load(&req.output);
    Ok(VideoEditResult {
        output_path: req.output.clone(),
        duration: info.as_ref().ok().map(|v| v.duration),
        size_bytes: file_size(&req.output),
    })
}

// ── video_extract_audio ────────────────────────────────────────────────────

/// Extract the audio track from a video as a WAV file (16 kHz mono for transcription).
pub fn video_extract_audio(req: &ExtractAudioRequest) -> Result<VideoEditResult, VideoEditError> {
    check_ffmpeg()?;
    run_ffmpeg(&[
        "-y",
        "-i",
        &req.input,
        "-vn",
        "-acodec",
        "pcm_s16le",
        "-ar",
        "16000",
        "-ac",
        "1",
        &req.output,
    ])?;
    Ok(VideoEditResult {
        output_path: req.output.clone(),
        duration: None,
        size_bytes: file_size(&req.output),
    })
}

// ── video_remove_silence ───────────────────────────────────────────────────

/// Detect and remove silent sections from a video using FFmpeg `silencedetect`.
pub fn video_remove_silence(req: &RemoveSilenceRequest) -> Result<VideoEditResult, VideoEditError> {
    check_ffmpeg()?;

    // Step 1: Detect silence
    let filter = format!(
        "silencedetect=noise={}dB:d={}",
        req.threshold_db, req.min_silence_secs
    );
    let probe_output = Command::new("ffmpeg")
        .args(["-i", &req.input, "-af", &filter, "-f", "null", "-"])
        .output()
        .map_err(|_| VideoEditError::FfmpegNotFound)?;

    // silencedetect prints to stderr
    let stderr = String::from_utf8_lossy(&probe_output.stderr).to_string();
    let silence_ranges = parse_silence_ranges(&stderr);

    // Step 2: Invert ranges to find keep segments
    let info = video_load(&req.input)?;
    let keep_segments = invert_silence_ranges(&silence_ranges, info.duration);

    if keep_segments.is_empty() {
        // Nothing to remove — just copy
        run_ffmpeg(&["-y", "-i", &req.input, "-c", "copy", &req.output])?;
        return Ok(VideoEditResult {
            output_path: req.output.clone(),
            duration: Some(info.duration),
            size_bytes: file_size(&req.output),
        });
    }

    // Step 3: Trim keep segments
    let tmp_dir = std::env::temp_dir().join("mint_silence_removal");
    let _ = std::fs::create_dir_all(&tmp_dir);

    let mut segment_paths: Vec<String> = Vec::new();
    for (i, (start, end)) in keep_segments.iter().enumerate() {
        let seg_path = tmp_dir.join(format!("seg_{i:04}.mp4"));
        let seg_str = seg_path.to_string_lossy().to_string();
        let trim_req = TrimRequest {
            input: req.input.clone(),
            output: seg_str.clone(),
            start: *start,
            end: *end,
        };
        video_trim(&trim_req)?;
        segment_paths.push(seg_str);
    }

    // Step 4: Merge segments
    let merge_req = MergeRequest {
        inputs: segment_paths.clone(),
        output: req.output.clone(),
    };
    let result = video_merge(&merge_req)?;

    // Cleanup temp segments
    for seg in &segment_paths {
        let _ = std::fs::remove_file(seg);
    }
    let _ = std::fs::remove_dir(&tmp_dir);

    Ok(result)
}

/// Parse `silence_start` / `silence_end` lines from ffmpeg silencedetect stderr.
pub fn parse_silence_ranges(stderr: &str) -> Vec<(f64, f64)> {
    let mut ranges = Vec::new();
    let mut current_start: Option<f64> = None;

    for line in stderr.lines() {
        if line.contains("silence_start:") {
            if let Some(val) = extract_float_after(line, "silence_start:") {
                current_start = Some(val);
            }
        } else if line.contains("silence_end:") {
            if let Some(end) = extract_float_after(line, "silence_end:") {
                if let Some(start) = current_start.take() {
                    ranges.push((start, end));
                }
            }
        }
    }
    ranges
}

fn extract_float_after(line: &str, after: &str) -> Option<f64> {
    let idx = line.find(after)?;
    let rest = line[idx + after.len()..].trim();
    let word: &str = rest.split_whitespace().next()?;
    word.parse::<f64>().ok()
}

/// Convert silence ranges to non-silent "keep" segments.
pub fn invert_silence_ranges(silences: &[(f64, f64)], total_duration: f64) -> Vec<(f64, f64)> {
    let mut segments = Vec::new();
    let mut cursor = 0.0f64;
    for &(silence_start, silence_end) in silences {
        if cursor < silence_start {
            segments.push((cursor, silence_start));
        }
        cursor = silence_end;
    }
    if cursor < total_duration {
        segments.push((cursor, total_duration));
    }
    segments
}

// ── video_export ───────────────────────────────────────────────────────────

/// Re-encode a video to the requested resolution/fps/codec.
pub fn video_export(req: &ExportRequest) -> Result<VideoEditResult, VideoEditError> {
    check_ffmpeg()?;

    let codec = req
        .codec
        .as_deref()
        .filter(|s| !s.is_empty())
        .unwrap_or("libx264");
    let crf = req.crf.unwrap_or(23).to_string();

    let mut args: Vec<String> = vec![
        "-y".into(),
        "-i".into(),
        req.input.clone(),
        "-c:v".into(),
        codec.to_string(),
        "-crf".into(),
        crf,
        "-preset".into(),
        "fast".into(),
        "-c:a".into(),
        "aac".into(),
        "-b:a".into(),
        "192k".into(),
    ];

    if let Some(fps) = req.fps {
        args.push("-r".into());
        args.push(fps.to_string());
    }

    if let Some(res) = &req.resolution {
        if let Some((w, h)) = res.split_once('x') {
            args.push("-vf".into());
            args.push(format!("scale={}:{}", w, h));
        }
    }

    args.push(req.output.clone());

    let str_args: Vec<&str> = args.iter().map(String::as_str).collect();
    run_ffmpeg(&str_args)?;

    let info = video_load(&req.output);
    Ok(VideoEditResult {
        output_path: req.output.clone(),
        duration: info.as_ref().ok().map(|v| v.duration),
        size_bytes: file_size(&req.output),
    })
}

// ── Audio Ducking & Speaker Zoom & Timeline Reorder ────────────────────────

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DuckMusicRequest {
    pub video_input: String,
    pub music_input: String,
    pub output: String,
    #[serde(default = "default_duck_factor")]
    pub music_volume: f32,
}

fn default_duck_factor() -> f32 { 0.2 }

pub fn audio_duck_music(req: &DuckMusicRequest) -> Result<VideoEditResult, VideoEditError> {
    let args = vec![
        "-y".to_string(),
        "-i".to_string(), req.video_input.clone(),
        "-i".to_string(), req.music_input.clone(),
        "-filter_complex".to_string(),
        format!("[1:a]volume={}[bg];[0:a][bg]sidechaincompress=threshold=0.08:ratio=5:attack=150:release=800[aout]", req.music_volume),
        "-map".to_string(), "0:v".to_string(),
        "-map".to_string(), "[aout]".to_string(),
        "-c:v".to_string(), "copy".to_string(),
        req.output.clone(),
    ];
    let str_args: Vec<&str> = args.iter().map(String::as_str).collect();
    run_ffmpeg(&str_args)?;
    let info = video_load(&req.output);
    Ok(VideoEditResult {
        output_path: req.output.clone(),
        duration: info.as_ref().ok().map(|v| v.duration),
        size_bytes: file_size(&req.output),
    })
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ZoomSpeakerRequest {
    pub input: String,
    pub output: String,
    #[serde(default = "default_zoom_factor")]
    pub zoom_factor: f32,
}

fn default_zoom_factor() -> f32 { 1.25 }

pub fn effect_zoom_on_speaker(req: &ZoomSpeakerRequest) -> Result<VideoEditResult, VideoEditError> {
    let zoom = req.zoom_factor.max(1.0).min(3.0);
    let crop_filter = format!("crop=w=iw/{zoom}:h=ih/{zoom}:x=(iw-w)/2:y=(ih-h)/2,scale=1920:1080");
    let args = vec![
        "-y".to_string(),
        "-i".to_string(), req.input.clone(),
        "-vf".to_string(), crop_filter,
        "-c:a".to_string(), "copy".to_string(),
        req.output.clone(),
    ];
    let str_args: Vec<&str> = args.iter().map(String::as_str).collect();
    run_ffmpeg(&str_args)?;
    let info = video_load(&req.output);
    Ok(VideoEditResult {
        output_path: req.output.clone(),
        duration: info.as_ref().ok().map(|v| v.duration),
        size_bytes: file_size(&req.output),
    })
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReorderClipsRequest {
    pub inputs: Vec<String>,
    pub order: Vec<usize>,
    pub output: String,
}

pub fn timeline_reorder(req: &ReorderClipsRequest) -> Result<VideoEditResult, VideoEditError> {
    let mut reordered_inputs = Vec::new();
    for &idx in &req.order {
        if idx < req.inputs.len() {
            reordered_inputs.push(req.inputs[idx].clone());
        }
    }
    if reordered_inputs.is_empty() {
        reordered_inputs = req.inputs.clone();
    }

    video_merge(&MergeRequest {
        inputs: reordered_inputs,
        output: req.output.clone(),
    })
}

// ── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_fraction() {
        assert!((parse_fraction("30000/1001").unwrap() - 29.97).abs() < 0.01);
        assert!((parse_fraction("25/1").unwrap() - 25.0).abs() < 0.001);
        assert!((parse_fraction("30.0").unwrap() - 30.0).abs() < 0.001);
    }

    #[test]
    fn test_invert_silence_none() {
        let segs = invert_silence_ranges(&[], 10.0);
        assert_eq!(segs, vec![(0.0, 10.0)]);
    }

    #[test]
    fn test_invert_silence_middle() {
        let segs = invert_silence_ranges(&[(3.0, 5.0)], 10.0);
        assert_eq!(segs, vec![(0.0, 3.0), (5.0, 10.0)]);
    }

    #[test]
    fn test_invert_silence_at_start() {
        let segs = invert_silence_ranges(&[(0.0, 2.0)], 10.0);
        assert_eq!(segs, vec![(2.0, 10.0)]);
    }

    #[test]
    fn test_parse_silence_ranges() {
        let stderr = "\
[silencedetect] silence_start: 1.5\n\
[silencedetect] silence_end: 3.2 | silence_duration: 1.7\n\
[silencedetect] silence_start: 7.0\n\
[silencedetect] silence_end: 9.5 | silence_duration: 2.5\n";
        let ranges = parse_silence_ranges(stderr);
        assert_eq!(ranges.len(), 2);
        assert!((ranges[0].0 - 1.5).abs() < 0.001);
        assert!((ranges[0].1 - 3.2).abs() < 0.001);
    }
}

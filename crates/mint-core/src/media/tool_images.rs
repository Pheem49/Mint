//! One bounded image path for browser screenshots and view_image. Original
//! bytes are durable artifacts; model images may be resized independently.
use crate::mcp_result::{McpArtifact, ToolOutcome};
use base64::{Engine as _, engine::general_purpose::STANDARD};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::{fs, io::Cursor};

pub const ORIGINAL_LIMIT: usize = 20 * 1024 * 1024;
pub const PIXEL_LIMIT: u64 = 32_000_000;
pub const MODEL_LIMIT: usize = 2_000_000;
pub const MAX_MODEL_DIMENSION: u32 = 1280;
pub const MODEL_JPEG_QUALITY: u8 = 85;

pub struct PreparedImage {
    pub data_uri: String,
    pub width: u32,
    pub height: u32,
    pub reduced: bool,
}

pub fn prepare(bytes: &[u8], mime: &str) -> Result<PreparedImage, String> {
    if bytes.len() > ORIGINAL_LIMIT {
        return Err("Image exceeds 20 MiB; capture the viewport or one element instead.".into());
    }
    // Preserve legacy small GIF/WebP/SVG attachments. PNG/JPEG, including all
    // browser captures, receive decoded dimension and memory checks below.
    if matches!(mime, "image/gif" | "image/webp" | "image/svg+xml") {
        if bytes.len() > MODEL_LIMIT {
            return Err("Convert this image to PNG/JPEG to resize it for model input.".into());
        }
        return Ok(PreparedImage {
            data_uri: format!("data:{mime};base64,{}", STANDARD.encode(bytes)),
            width: 0,
            height: 0,
            reduced: false,
        });
    }
    let format = match mime {
        "image/png" => image::ImageFormat::Png,
        "image/jpeg" => image::ImageFormat::Jpeg,
        _ => return Err("Unsupported image type".into()),
    };
    let (width, height) = image::ImageReader::with_format(Cursor::new(bytes), format)
        .into_dimensions()
        .map_err(|e| format!("Invalid image: {e}"))?;
    if width == 0 || height == 0 || u64::from(width) * u64::from(height) > PIXEL_LIMIT {
        return Err(
            "Image exceeds 32 million pixels; capture the viewport or one element instead.".into(),
        );
    }
    let image = image::ImageReader::with_format(Cursor::new(bytes), format)
        .decode()
        .map_err(|e| format!("Invalid image: {e}"))?;
    let max_dim = width.max(height);
    if bytes.len() <= MODEL_LIMIT && max_dim <= MAX_MODEL_DIMENSION {
        return Ok(PreparedImage {
            data_uri: format!("data:{mime};base64,{}", STANDARD.encode(bytes)),
            width,
            height,
            reduced: false,
        });
    }
    let mut size = MAX_MODEL_DIMENSION.min(max_dim);
    loop {
        let resized = image.thumbnail(size, size).to_rgb8();
        let mut encoded = Vec::new();
        image::codecs::jpeg::JpegEncoder::new_with_quality(&mut encoded, MODEL_JPEG_QUALITY)
            .encode_image(&resized)
            .map_err(|e| e.to_string())?;
        if encoded.len() <= MODEL_LIMIT {
            return Ok(PreparedImage {
                data_uri: format!("data:image/jpeg;base64,{}", STANDARD.encode(encoded)),
                width,
                height,
                reduced: true,
            });
        }
        if size <= 128 {
            return Err("Cannot prepare this image within the model image limit".into());
        }
        size = size * 3 / 4;
    }
}

/// Which stored image a `view_image` call refers to.
#[derive(Debug, PartialEq, Eq)]
pub enum ViewTarget<'a> {
    Artifact(&'a str),
    Path(&'a str),
}

/// Single place that interprets `view_image` arguments. An artifact wins when
/// both are given (it is already in the conversation); blank values count as
/// missing; supplying neither explains how to recover instead of just failing.
pub fn resolve_view_target<'a>(path: &'a str, artifact_id: &'a str) -> Result<ViewTarget<'a>, String> {
    let (path, artifact_id) = (path.trim(), artifact_id.trim());
    if !artifact_id.is_empty() {
        Ok(ViewTarget::Artifact(artifact_id))
    } else if !path.is_empty() {
        Ok(ViewTarget::Path(path))
    } else {
        Err("view_image needs a `path` (an image file, e.g. the --screenshot file you just wrote) \
             or an `artifactId` (from an earlier tool result). Images you already viewed in this \
             turn are already in your context; do not request them again."
            .into())
    }
}

/// Stable identity of image bytes, used to avoid attaching the same picture twice.
pub fn content_key(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    let mut key = String::from("sha256:");
    for byte in digest {
        key.push_str(&format!("{byte:02x}"));
    }
    key
}

fn metadata_with_hash(metadata: Option<Value>, bytes: &[u8]) -> Value {
    let mut metadata = metadata.unwrap_or_else(|| serde_json::json!({}));
    metadata["contentHash"] = content_key(bytes).into();
    metadata
}

pub fn save(
    bytes: &[u8],
    mime: &str,
    name: &str,
    chat: &str,
    call: &str,
    source: &str,
    metadata: Option<Value>,
) -> Result<ToolOutcome, String> {
    let prepared = prepare(bytes, mime)?;
    let mut metadata = metadata_with_hash(metadata, bytes);
    metadata["width"] = prepared.width.into();
    metadata["height"] = prepared.height.into();
    let entry = McpArtifact {
        id: uuid::Uuid::new_v4().to_string(),
        name: name.into(),
        mime_type: mime.into(),
        kind: "image".into(),
        size: bytes.len(),
        chat_id: chat.into(),
        call_id: call.into(),
        server: source.into(),
        uri: None,
        metadata: Some(metadata),
    };
    let directory = crate::mcp_result::artifact_directory()?;
    fs::create_dir_all(&directory).map_err(|e| e.to_string())?;
    fs::write(directory.join(&entry.id), bytes).map_err(|e| e.to_string())?;
    if let Err(error) = fs::write(
        directory.join(format!("{}.json", entry.id)),
        serde_json::to_vec(&entry).map_err(|e| e.to_string())?,
    ) {
        let _ = fs::remove_file(directory.join(&entry.id));
        return Err(error.to_string());
    }
    let warnings = if prepared.reduced {
        vec![
            "Model input was resized/compressed; the original image is available for download."
                .into(),
        ]
    } else {
        Vec::new()
    };
    Ok(ToolOutcome {
        text: format!(
            "Image saved: {} (artifactId: {}). Original: {} × {}.",
            entry.name, entry.id, prepared.width, prepared.height
        ),
        images: vec![prepared.data_uri],
        artifacts: vec![entry],
        warnings,
        ..Default::default()
    })
}

pub fn view_artifact(id: &str, chat: &str) -> Result<ToolOutcome, String> {
    let (entry, bytes) = crate::mcp_result::read_artifact(id)?;
    if entry.chat_id != chat || entry.kind != "image" {
        return Err("Image artifact is not available in this conversation.".into());
    }
    let prepared = prepare(&bytes, &entry.mime_type)?;
    Ok(ToolOutcome {
        text: format!("Image loaded: {} (artifactId: {})", entry.name, entry.id),
        images: vec![prepared.data_uri],
        artifacts: vec![entry],
        ..Default::default()
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn view_target_prefers_artifact_when_both_are_supplied() {
        assert_eq!(
            resolve_view_target("/tmp/a.png", "abc").unwrap(),
            ViewTarget::Artifact("abc")
        );
    }

    #[test]
    fn view_target_accepts_either_single_argument_ignoring_blank_whitespace() {
        assert_eq!(
            resolve_view_target("  /tmp/a.png ", "  ").unwrap(),
            ViewTarget::Path("/tmp/a.png")
        );
        assert_eq!(
            resolve_view_target("", "abc").unwrap(),
            ViewTarget::Artifact("abc")
        );
    }

    #[test]
    fn empty_view_target_error_tells_the_model_what_to_do() {
        let error = resolve_view_target("", " ").unwrap_err();
        assert!(error.contains("path"), "{error}");
        assert!(error.contains("artifactId"), "{error}");
        assert!(error.contains("already"), "{error}");
    }

    #[test]
    fn content_key_is_stable_and_content_sensitive() {
        assert_eq!(content_key(b"abc"), content_key(b"abc"));
        assert_ne!(content_key(b"abc"), content_key(b"abd"));
        assert!(content_key(b"abc").starts_with("sha256:"));
    }

    #[test]
    fn saved_artifact_metadata_records_the_content_hash() {
        let mut bytes = Cursor::new(Vec::new());
        image::RgbImage::new(4, 4)
            .write_to(&mut bytes, image::ImageFormat::Png)
            .unwrap();
        let metadata = metadata_with_hash(None, bytes.get_ref());
        assert_eq!(metadata["contentHash"], content_key(bytes.get_ref()));
    }

    #[test]
    fn small_images_under_max_dimension_are_not_reduced() {
        let mut bytes = Cursor::new(Vec::new());
        image::RgbImage::new(800, 600)
            .write_to(&mut bytes, image::ImageFormat::Png)
            .unwrap();
        let prepared = prepare(bytes.get_ref(), "image/png").unwrap();
        assert_eq!(prepared.width, 800);
        assert_eq!(prepared.height, 600);
        assert!(!prepared.reduced);
        assert!(prepared.data_uri.starts_with("data:image/png;base64,"));
    }

    #[test]
    fn large_images_exceeding_max_dimension_are_downscaled() {
        let mut bytes = Cursor::new(Vec::new());
        image::RgbImage::new(2000, 1000)
            .write_to(&mut bytes, image::ImageFormat::Png)
            .unwrap();
        let prepared = prepare(bytes.get_ref(), "image/png").unwrap();
        assert_eq!(prepared.width, 2000);
        assert_eq!(prepared.height, 1000);
        assert!(prepared.reduced);
        assert!(prepared.data_uri.starts_with("data:image/jpeg;base64,"));

        // Verify the encoded image inside data_uri is capped at MAX_MODEL_DIMENSION
        let raw_b64 = prepared
            .data_uri
            .trim_start_matches("data:image/jpeg;base64,");
        let decoded_bytes = STANDARD.decode(raw_b64).unwrap();
        let (w, h) =
            image::ImageReader::with_format(Cursor::new(decoded_bytes), image::ImageFormat::Jpeg)
                .into_dimensions()
                .unwrap();
        assert!(w <= MAX_MODEL_DIMENSION && h <= MAX_MODEL_DIMENSION);
        assert_eq!(w, MAX_MODEL_DIMENSION);
        assert_eq!(h, MAX_MODEL_DIMENSION / 2);
    }
}

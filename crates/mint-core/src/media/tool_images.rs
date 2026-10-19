//! One bounded image path for browser screenshots and view_image. Original
//! bytes are durable artifacts; model images may be resized independently.
use crate::mcp_result::{McpArtifact, ToolOutcome};
use base64::{Engine as _, engine::general_purpose::STANDARD};
use serde_json::Value;
use std::{fs, io::Cursor};

pub const ORIGINAL_LIMIT: usize = 20 * 1024 * 1024;
pub const PIXEL_LIMIT: u64 = 32_000_000;
pub const MODEL_LIMIT: usize = 2_000_000;

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
    if bytes.len() <= MODEL_LIMIT {
        return Ok(PreparedImage {
            data_uri: format!("data:{mime};base64,{}", STANDARD.encode(bytes)),
            width,
            height,
            reduced: false,
        });
    }
    let mut size = 2048u32.min(width.max(height));
    loop {
        let resized = image.thumbnail(size, size).to_rgb8();
        let mut encoded = Vec::new();
        image::codecs::jpeg::JpegEncoder::new_with_quality(&mut encoded, 85)
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
    let mut metadata = metadata.unwrap_or_else(|| serde_json::json!({}));
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

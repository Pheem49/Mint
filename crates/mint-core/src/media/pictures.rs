use std::{
    fs,
    io::Cursor,
    path::{Path, PathBuf},
    process::Command,
};

use base64::{Engine as _, engine::general_purpose::STANDARD};
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::ConfigError;

const THUMBNAIL_MAX_SIZE: u32 = 360;

#[derive(Debug, Error)]
pub enum PictureError {
    #[error(transparent)]
    Library(#[from] super::library::LibraryError),
    #[error(transparent)]
    Config(#[from] ConfigError),
    #[error("unable to create Pictures directory {path}: {source}")]
    CreateDirectory {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("unable to read Pictures index {path}: {source}")]
    ReadIndex {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("unable to parse Pictures index {path}: {source}")]
    ParseIndex {
        path: PathBuf,
        source: serde_json::Error,
    },
    #[error("unable to save picture {path}: {source}")]
    WritePicture {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("unable to serialize Pictures index: {0}")]
    SerializeIndex(serde_json::Error),
    #[error("unable to write Pictures index {path}: {source}")]
    WriteIndex {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("picture with id {0} not found")]
    NotFound(String),
    #[error("invalid or unsupported image data URI")]
    InvalidDataUri,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PictureEntry {
    pub id: String,
    pub filename: String,
    pub path: PathBuf,
    pub mime_type: String,
    pub created_at: String,
    pub source: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub thumbnail_path: Option<PathBuf>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub thumbnail_url: Option<String>,
}

pub fn list_saved_pictures() -> Result<Vec<PictureEntry>, PictureError> {
    let mut entries = super::library::LibraryStore::open_default()?.all_media()?;
    for entry in &mut entries {
        entry.url = Some(file_url(&entry.path));
        entry.thumbnail_url = entry.thumbnail_path.as_deref().map(file_url);
    }
    Ok(entries)
}

pub fn delete_saved_picture(id: &str) -> Result<(), PictureError> {
    let store = super::library::LibraryStore::open_default()?;
    let item = if let Ok(item) = store.get(id) {
        item
    } else {
        let entry = store
            .all_media()?
            .into_iter()
            .find(|entry| entry.filename == id || entry.path.to_string_lossy() == id)
            .ok_or_else(|| PictureError::NotFound(id.into()))?;
        store.get(&entry.id)?
    };
    store.set_trashed(&item.id, true)?;
    Ok(())
}

pub fn save_chat_images(
    images: Vec<String>,
    source: Option<String>,
    message: Option<String>,
) -> Result<Vec<PictureEntry>, PictureError> {
    save_chat_images_in(
        super::library::LibraryStore::open_default()?,
        images,
        source,
        message,
    )
}

pub(crate) fn save_chat_images_in(
    store: super::library::LibraryStore,
    images: Vec<String>,
    source: Option<String>,
    message: Option<String>,
) -> Result<Vec<PictureEntry>, PictureError> {
    let source = source.as_deref().unwrap_or("chat");
    let files: Vec<_> = images
        .into_iter()
        .enumerate()
        .filter_map(|(index, data_uri)| {
            let (_, extension, bytes) = parse_data_uri(&data_uri)?;
            if bytes.is_empty() {
                return None;
            }
            Some(super::library::LibraryUpload {
                name: format!("Media {}.{extension}", index + 1),
                data_uri,
            })
        })
        .collect();
    if files.is_empty() {
        return Ok(vec![]);
    }
    let origin = if matches!(source, "chat" | "web" | "cli" | "upload") {
        "uploaded"
    } else {
        "generated"
    };
    Ok(store
        .import(
            &files,
            &uuid::Uuid::new_v4().to_string(),
            None,
            origin,
            source,
            message.as_deref().unwrap_or_default(),
        )?
        .into_iter()
        .map(super::library::picture_entry)
        .collect())
}

pub fn save_sent_image(data_uri: &str, message: &str) -> Result<PictureEntry, PictureError> {
    save_chat_images(
        vec![data_uri.to_owned()],
        Some("cli".into()),
        Some(picture_message(message)),
    )?
    .into_iter()
    .next()
    .ok_or(PictureError::InvalidDataUri)
}

pub fn parse_data_uri(raw: &str) -> Option<(String, &'static str, Vec<u8>)> {
    let payload = raw.strip_prefix("data:")?;
    let (mime_type, encoded) = payload.split_once(";base64,")?;
    let mime_type = mime_type.to_ascii_lowercase();
    let extension = match mime_type.as_str() {
        "image/png" => "png",
        "image/jpeg" | "image/jpg" => "jpg",
        "image/webp" => "webp",
        "image/gif" => "gif",
        "video/mp4" => "mp4",
        "video/webm" => "webm",
        "video/quicktime" => "mov",
        "video/x-msvideo" => "avi",
        "video/x-matroska" => "mkv",
        _ => return None,
    };
    Some((mime_type, extension, STANDARD.decode(encoded).ok()?))
}

fn ensure_directory(directory: &Path) -> Result<(), PictureError> {
    fs::create_dir_all(directory).map_err(|source| PictureError::CreateDirectory {
        path: directory.into(),
        source,
    })
}

fn thumbnails_directory(directory: &Path) -> PathBuf {
    directory.join("thumbnails")
}

pub(crate) fn create_thumbnail_from_bytes(
    directory: &Path,
    id: &str,
    bytes: &[u8],
) -> Option<PathBuf> {
    let image = image::load_from_memory(bytes).ok()?;
    let thumbnail = image.thumbnail(THUMBNAIL_MAX_SIZE, THUMBNAIL_MAX_SIZE);
    let thumbnail_directory = thumbnails_directory(directory);
    ensure_directory(&thumbnail_directory).ok()?;

    let path = thumbnail_directory.join(format!("{id}.thumb.png"));
    let mut encoded = Cursor::new(Vec::new());
    thumbnail
        .write_to(&mut encoded, image::ImageFormat::Png)
        .ok()?;
    fs::write(&path, encoded.into_inner()).ok()?;
    Some(path)
}

pub(crate) fn extract_video_thumbnail_from_bytes(
    directory: &Path,
    id: &str,
    bytes: &[u8],
) -> Option<PathBuf> {
    // Write video to temporary file
    let temp_video = std::env::temp_dir().join(format!("mint-video-{}.tmp", id));
    if fs::write(&temp_video, bytes).is_err() {
        return None;
    }

    let temp_frame = std::env::temp_dir().join(format!("mint-frame-{}.png", id));

    let res = (|| -> Option<PathBuf> {
        let temp_video_str = temp_video.to_str()?;
        let temp_frame_str = temp_frame.to_str()?;

        let output = Command::new("ffmpeg")
            .args(&["-i", temp_video_str])
            .args(&["-vf", "select=eq(n\\,0)"])
            .args(&["-vframes", "1"])
            .args(&["-y", temp_frame_str])
            .stderr(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .output()
            .ok()?;

        if !output.status.success() {
            return None;
        }

        // Load extracted frame and create thumbnail
        let frame_bytes = fs::read(&temp_frame).ok()?;
        let image = image::load_from_memory(&frame_bytes).ok()?;
        let thumbnail = image.thumbnail(THUMBNAIL_MAX_SIZE, THUMBNAIL_MAX_SIZE);

        let thumbnail_directory = thumbnails_directory(directory);
        ensure_directory(&thumbnail_directory).ok()?;
        let path = thumbnail_directory.join(format!("{id}.thumb.png"));

        let mut encoded = Cursor::new(Vec::new());
        thumbnail
            .write_to(&mut encoded, image::ImageFormat::Png)
            .ok()?;
        fs::write(&path, encoded.into_inner()).ok()?;
        Some(path)
    })();

    // Always clean up temp files to avoid leaking in /tmp
    let _ = fs::remove_file(&temp_video);
    let _ = fs::remove_file(&temp_frame);

    res
}

fn picture_message(message: &str) -> String {
    let message = message.trim();
    if message.contains("[Image #1]") {
        message.into()
    } else if message.is_empty() {
        "[Image #1]".into()
    } else {
        format!("{message} [Image #1]")
    }
}

fn file_url(path: &Path) -> String {
    format!("file://{}", encode_url_path(&path.to_string_lossy()))
}

fn encode_url_path(path: &str) -> String {
    path.bytes()
        .map(|byte| match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' | b'/' => {
                (byte as char).to_string()
            }
            _ => format!("%{byte:02X}"),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn escapes_file_url_spaces() {
        assert_eq!(
            file_url(Path::new("/tmp/mint image.png")),
            "file:///tmp/mint%20image.png"
        );
    }
}

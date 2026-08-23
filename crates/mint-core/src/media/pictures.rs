use std::{
    fs,
    io::Cursor,
    path::{Path, PathBuf},
    process::Command,
    time::{SystemTime, UNIX_EPOCH},
};

use base64::{Engine as _, engine::general_purpose::STANDARD};
use chrono::{SecondsFormat, Utc};
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::{ConfigError, config_path};

const THUMBNAIL_MAX_SIZE: u32 = 360;

#[derive(Debug, Error)]
pub enum PictureError {
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
    let directory = pictures_directory()?;
    ensure_directory(&directory)?;
    let mut changed = false;
    let entries = read_index(&directory)?
        .into_iter()
        .filter(|entry| entry.path.exists())
        .map(|mut entry| {
            if entry
                .thumbnail_path
                .as_ref()
                .is_none_or(|thumbnail_path| !thumbnail_path.exists())
                && let Some(thumbnail_path) = create_thumbnail_from_file(&directory, &entry)
            {
                entry.thumbnail_path = Some(thumbnail_path);
                changed = true;
            }
            entry.url = Some(file_url(&entry.path));
            entry.thumbnail_url = entry.thumbnail_path.as_deref().map(file_url);
            entry
        })
        .collect::<Vec<_>>();

    if changed {
        write_index(&directory, &entries)?;
    }

    Ok(entries)
}

pub fn delete_saved_picture(id: &str) -> Result<(), PictureError> {
    let directory = pictures_directory()?;
    ensure_directory(&directory)?;
    let mut entries = read_index(&directory)?;

    let target_index = entries.iter().position(|entry| {
        entry.id == id
            || entry.filename == id
            || entry.path.file_name().and_then(|f| f.to_str()) == Some(id)
            || entry.path.to_string_lossy() == id
    });

    if let Some(index) = target_index {
        let entry = entries.remove(index);
        if entry.path.exists() {
            let _ = fs::remove_file(&entry.path);
        }
        if let Some(ref thumbnail_path) = entry.thumbnail_path {
            if thumbnail_path.exists() {
                let _ = fs::remove_file(thumbnail_path);
            }
        }
        write_index(&directory, &entries)?;
        Ok(())
    } else {
        Err(PictureError::NotFound(id.to_string()))
    }
}

pub fn save_chat_images(
    images: Vec<String>,
    source: Option<String>,
    message: Option<String>,
) -> Result<Vec<PictureEntry>, PictureError> {
    save_chat_images_to_directory(
        &pictures_directory()?,
        images,
        source.as_deref().unwrap_or("chat"),
        message.as_deref().unwrap_or_default(),
    )
}

pub fn save_sent_image(data_uri: &str, message: &str) -> Result<PictureEntry, PictureError> {
    save_chat_images_to_directory(
        &pictures_directory()?,
        vec![data_uri.to_owned()],
        "cli",
        &picture_message(message),
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

fn save_chat_images_to_directory(
    directory: &Path,
    images: Vec<String>,
    source: &str,
    message: &str,
) -> Result<Vec<PictureEntry>, PictureError> {
    ensure_directory(directory)?;
    let mut index = read_index(directory)?;
    let mut saved = Vec::new();
    for image in images {
        let Some((mime_type, extension, bytes)) = parse_data_uri(&image) else {
            continue;
        };
        if bytes.is_empty() {
            continue;
        }
        let filename = format!(
            "mint-{}-{}.{}",
            timestamp_for_filename(),
            unique_suffix(),
            extension
        );
        let path = directory.join(&filename);
        fs::write(&path, &bytes).map_err(|source| PictureError::WritePicture {
            path: path.clone(),
            source,
        })?;
        let id = filename
            .trim_end_matches(&format!(".{extension}"))
            .to_owned();
        let thumbnail_path = if mime_type.starts_with("image/") {
            create_thumbnail_from_bytes(directory, &id, &bytes)
        } else if mime_type.starts_with("video/") {
            extract_video_thumbnail_from_bytes(directory, &id, &bytes)
        } else {
            None
        };
        let entry = PictureEntry {
            id,
            filename,
            path,
            mime_type,
            created_at: Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true),
            source: source.into(),
            message: message.chars().take(240).collect(),
            thumbnail_path,
            url: None,
            thumbnail_url: None,
        };
        index.insert(0, entry.clone());
        saved.push(entry);
    }
    write_index(directory, &index)?;
    Ok(saved)
}

fn pictures_directory() -> Result<PathBuf, PictureError> {
    Ok(config_path()?.with_file_name("Pictures"))
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

fn create_thumbnail_from_file(directory: &Path, entry: &PictureEntry) -> Option<PathBuf> {
    let bytes = fs::read(&entry.path).ok()?;
    create_thumbnail_from_bytes(directory, &entry.id, &bytes)
}

fn create_thumbnail_from_bytes(directory: &Path, id: &str, bytes: &[u8]) -> Option<PathBuf> {
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

fn extract_video_thumbnail_from_bytes(directory: &Path, id: &str, bytes: &[u8]) -> Option<PathBuf> {
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

fn read_index(directory: &Path) -> Result<Vec<PictureEntry>, PictureError> {
    let path = directory.join("pictures.json");
    if !path.exists() {
        return Ok(Vec::new());
    }
    let raw = fs::read_to_string(&path).map_err(|source| PictureError::ReadIndex {
        path: path.clone(),
        source,
    })?;
    serde_json::from_str(&raw).map_err(|source| PictureError::ParseIndex { path, source })
}

fn write_index(directory: &Path, entries: &[PictureEntry]) -> Result<(), PictureError> {
    let path = directory.join("pictures.json");
    let raw = serde_json::to_string_pretty(entries).map_err(PictureError::SerializeIndex)?;
    fs::write(&path, format!("{raw}\n")).map_err(|source| PictureError::WriteIndex { path, source })
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

fn timestamp_for_filename() -> String {
    Utc::now().format("%Y%m%d-%H%M%S").to_string()
}

fn unique_suffix() -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    format!("{:08x}", (nanos ^ u128::from(std::process::id())) as u32)
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

    #[test]
    fn saves_sent_image_and_updates_index() {
        let directory =
            std::env::temp_dir().join(format!("mint-core-pictures-test-{}", unique_suffix()));
        let entries = save_chat_images_to_directory(
            &directory,
            vec!["data:image/png;base64,aGk=".into()],
            "cli",
            &picture_message("describe this"),
        )
        .unwrap();
        let index = read_index(&directory).unwrap();

        assert_eq!(fs::read(&entries[0].path).unwrap(), b"hi");
        assert_eq!(index.len(), 1);
        assert_eq!(index[0].mime_type, "image/png");
        assert_eq!(index[0].source, "cli");
        assert_eq!(index[0].message, "describe this [Image #1]");

        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn deletes_saved_picture_and_cleans_files_and_index() {
        let directory =
            std::env::temp_dir().join(format!("mint-core-pictures-del-test-{}", unique_suffix()));
        let entries = save_chat_images_to_directory(
            &directory,
            vec!["data:image/png;base64,aGk=".into()],
            "cli",
            &picture_message("test image"),
        )
        .unwrap();

        assert_eq!(entries.len(), 1);
        let picture_id = &entries[0].id;
        let picture_path = entries[0].path.clone();

        assert!(picture_path.exists());

        // Read index before delete
        let index_before = read_index(&directory).unwrap();
        assert_eq!(index_before.len(), 1);

        // Delete picture using read_index/write_index pattern
        let mut entries_to_mod = read_index(&directory).unwrap();
        let pos = entries_to_mod
            .iter()
            .position(|e| e.id == *picture_id)
            .unwrap();
        let removed = entries_to_mod.remove(pos);
        if removed.path.exists() {
            fs::remove_file(&removed.path).unwrap();
        }
        write_index(&directory, &entries_to_mod).unwrap();

        assert!(!picture_path.exists());
        let index_after = read_index(&directory).unwrap();
        assert_eq!(index_after.len(), 0);

        fs::remove_dir_all(directory).unwrap();
    }
}

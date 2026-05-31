use std::{
    fs,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use base64::{Engine as _, engine::general_purpose::STANDARD};
use mint_core::config_path;
use serde::{Deserialize, Serialize};

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
    pub url: Option<String>,
}

pub fn list_saved_pictures() -> Result<Vec<PictureEntry>, String> {
    ensure_directory()?;
    Ok(read_index()?
        .into_iter()
        .filter(|entry| entry.path.exists())
        .map(|mut entry| {
            entry.url = Some(file_url(&entry.path));
            entry
        })
        .collect())
}

pub fn save_chat_images(
    images: Vec<String>,
    source: Option<String>,
    message: Option<String>,
) -> Result<Vec<PictureEntry>, String> {
    ensure_directory()?;
    let mut index = read_index()?;
    let mut saved = Vec::new();
    for image in images {
        let Some((mime_type, extension, bytes)) = parse_data_uri(&image) else {
            continue;
        };
        if bytes.is_empty() {
            continue;
        }
        let filename = format!("mint-{}-{extension}", unique_id());
        let path = pictures_directory()?.join(&filename);
        fs::write(&path, bytes).map_err(|error| error.to_string())?;
        let entry = PictureEntry {
            id: filename.trim_end_matches(&format!(".{extension}")).into(),
            filename,
            path,
            mime_type,
            created_at: timestamp(),
            source: source.clone().unwrap_or_else(|| "chat".into()),
            message: message
                .clone()
                .unwrap_or_default()
                .chars()
                .take(240)
                .collect(),
            url: None,
        };
        index.insert(0, entry.clone());
        saved.push(entry);
    }
    write_index(&index)?;
    Ok(saved)
}

fn parse_data_uri(raw: &str) -> Option<(String, &'static str, Vec<u8>)> {
    let payload = raw.strip_prefix("data:")?;
    let (mime_type, encoded) = payload.split_once(";base64,")?;
    let extension = match mime_type.to_ascii_lowercase().as_str() {
        "image/png" => "png",
        "image/jpeg" | "image/jpg" => "jpg",
        "image/webp" => "webp",
        "image/gif" => "gif",
        _ => return None,
    };
    Some((
        mime_type.to_ascii_lowercase(),
        extension,
        STANDARD.decode(encoded).ok()?,
    ))
}

fn pictures_directory() -> Result<PathBuf, String> {
    Ok(config_path()
        .map_err(|error| error.to_string())?
        .with_file_name("Pictures"))
}

fn index_path() -> Result<PathBuf, String> {
    Ok(pictures_directory()?.join("pictures.json"))
}

fn ensure_directory() -> Result<(), String> {
    fs::create_dir_all(pictures_directory()?).map_err(|error| error.to_string())
}

fn read_index() -> Result<Vec<PictureEntry>, String> {
    let path = index_path()?;
    if !path.exists() {
        return Ok(Vec::new());
    }
    serde_json::from_str(&fs::read_to_string(path).map_err(|error| error.to_string())?)
        .map_err(|error| error.to_string())
}

fn write_index(entries: &[PictureEntry]) -> Result<(), String> {
    let raw = serde_json::to_string_pretty(entries).map_err(|error| error.to_string())?;
    fs::write(index_path()?, format!("{raw}\n")).map_err(|error| error.to_string())
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

fn unique_id() -> String {
    format!(
        "{}-{}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos(),
        std::process::id()
    )
}

fn timestamp() -> String {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_supported_image_data_uri() {
        let parsed = parse_data_uri("data:image/png;base64,aGk=").unwrap();
        assert_eq!(parsed.0, "image/png");
        assert_eq!(parsed.1, "png");
        assert_eq!(parsed.2, b"hi");
    }

    #[test]
    fn escapes_file_url_spaces() {
        assert_eq!(
            file_url(Path::new("/tmp/mint image.png")),
            "file:///tmp/mint%20image.png"
        );
    }
}

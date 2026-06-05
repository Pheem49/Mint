use std::path::Path;

use anyhow::{Context, Result, bail};
use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};

/// Load an image file from disk and return a `data:<mime>;base64,<data>` URI.
/// Supports PNG, JPEG, GIF, and WEBP by extension and magic bytes.
pub fn load_image_as_data_uri(path: &Path) -> Result<String> {
    let bytes = std::fs::read(path)
        .with_context(|| format!("failed to read image file: {}", path.display()))?;

    if bytes.is_empty() {
        bail!("image file is empty: {}", path.display());
    }

    let mime = detect_mime(&bytes, path);
    let encoded = BASE64.encode(&bytes);
    Ok(format!("data:{mime};base64,{encoded}"))
}

/// Try to read an image from the system clipboard.
/// Returns `None` if the clipboard contains no image or the required tool is unavailable.
/// On Linux uses `xclip` or `wl-paste`. On macOS uses `osascript`.
pub fn read_clipboard_image() -> Result<Option<String>> {
    #[cfg(target_os = "linux")]
    {
        if let Ok(output) = std::process::Command::new("xclip")
            .args(["-selection", "clipboard", "-t", "image/png", "-o"])
            .output()
        {
            if output.status.success() && !output.stdout.is_empty() {
                let encoded = BASE64.encode(&output.stdout);
                return Ok(Some(format!("data:image/png;base64,{encoded}")));
            }
        }

        if let Ok(output) = std::process::Command::new("wl-paste")
            .args(["--type", "image/png"])
            .output()
        {
            if output.status.success() && !output.stdout.is_empty() {
                let encoded = BASE64.encode(&output.stdout);
                return Ok(Some(format!("data:image/png;base64,{encoded}")));
            }
        }

        Ok(None)
    }

    #[cfg(target_os = "macos")]
    {
        let script = r#"
            set png_data to (the clipboard as «class PNGf»)
            return png_data
        "#;
        let output = std::process::Command::new("osascript")
            .args(["-e", script])
            .output();
        if let Ok(out) = output {
            if out.status.success() && !out.stdout.is_empty() {
                let encoded = BASE64.encode(&out.stdout);
                return Ok(Some(format!("data:image/png;base64,{encoded}")));
            }
        }
        Ok(None)
    }

    #[cfg(not(any(target_os = "linux", target_os = "macos")))]
    {
        Ok(None)
    }
}

pub fn save_sent_image_after_send(data_uri: Option<&str>, message: &str) {
    if let Some(data_uri) = data_uri {
        for img in data_uri.split_whitespace() {
            match mint_core::save_sent_image(img, message) {
                Ok(entry) => println!("\x1b[90mSaved image: {}\x1b[0m", entry.path.display()),
                Err(error) => eprintln!("\x1b[33mWarning: failed to save sent image: {error}\x1b[0m"),
            }
        }
    }
}

fn detect_mime(bytes: &[u8], path: &Path) -> &'static str {
    if bytes.starts_with(&[0x89, 0x50, 0x4E, 0x47]) {
        return "image/png";
    }
    if bytes.starts_with(&[0xFF, 0xD8, 0xFF]) {
        return "image/jpeg";
    }
    if bytes.starts_with(b"GIF8") {
        return "image/gif";
    }
    if bytes.starts_with(b"RIFF") && bytes.len() > 12 && &bytes[8..12] == b"WEBP" {
        return "image/webp";
    }

    match path
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_ascii_lowercase())
        .as_deref()
    {
        Some("jpg") | Some("jpeg") => "image/jpeg",
        Some("gif") => "image/gif",
        Some("webp") => "image/webp",
        Some("bmp") => "image/bmp",
        _ => "image/png",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_png_by_magic_bytes() {
        let png_magic = [0x89u8, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A];
        assert_eq!(
            detect_mime(&png_magic, Path::new("test.unknown")),
            "image/png"
        );
    }

    #[test]
    fn detects_jpeg_by_magic_bytes() {
        let jpeg_magic = [0xFF, 0xD8, 0xFF, 0xE0];
        assert_eq!(
            detect_mime(&jpeg_magic, Path::new("photo.jpg")),
            "image/jpeg"
        );
    }

    #[test]
    fn falls_back_to_extension() {
        let no_magic = [0x00u8, 0x01, 0x02, 0x03];
        assert_eq!(detect_mime(&no_magic, Path::new("anim.gif")), "image/gif");
    }
}

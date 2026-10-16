//! Normalize MCP results once, keeping binary payloads out of observations/history.
use base64::{Engine as _, engine::general_purpose::STANDARD};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    fs,
    path::{Path, PathBuf},
};

const ITEM_LIMIT: usize = 20 * 1024 * 1024;
const RESULT_LIMIT: usize = 50 * 1024 * 1024;

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ToolStatus {
    #[default]
    Success,
    Failed,
    TimedOut,
    Cancelled,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpArtifact {
    pub id: String,
    pub name: String,
    pub mime_type: String,
    pub kind: String,
    pub size: usize,
    pub chat_id: String,
    pub call_id: String,
    pub server: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub uri: Option<String>,
}

#[derive(Debug, Default)]
pub struct ToolOutcome {
    pub text: String,
    pub status: ToolStatus,
    pub images: Vec<String>,
    pub artifacts: Vec<McpArtifact>,
    pub warnings: Vec<String>,
    /// A dispatched Device command may still be running; suppress automatic repeats.
    pub completion_unconfirmed: bool,
}

impl From<String> for ToolOutcome {
    fn from(text: String) -> Self {
        let status = if text.starts_with("Error:")
            || text.starts_with("Blocked")
            || text.starts_with("User denied")
        {
            ToolStatus::Failed
        } else {
            ToolStatus::Success
        };
        Self {
            text,
            status,
            ..Self::default()
        }
    }
}

pub fn artifact_directory() -> Result<PathBuf, String> {
    crate::config_path()
        .map_err(|e| e.to_string())?
        .parent()
        .map(|p| p.join("mcp-artifacts"))
        .ok_or_else(|| "MCP artifact directory unavailable".into())
}

pub fn read_artifact(id: &str) -> Result<(McpArtifact, Vec<u8>), String> {
    read_artifact_in(&artifact_directory()?, id)
}

fn read_artifact_in(directory: &Path, id: &str) -> Result<(McpArtifact, Vec<u8>), String> {
    uuid::Uuid::parse_str(id).map_err(|_| "Invalid artifact ID")?;
    let entry: McpArtifact = serde_json::from_slice(
        &fs::read(directory.join(format!("{id}.json"))).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    if entry.id != id || entry.uri.is_some() {
        return Err("Artifact unavailable".into());
    }
    let bytes = fs::read(directory.join(id)).map_err(|e| e.to_string())?;
    Ok((entry, bytes))
}

pub fn normalize_mcp_result(result: Value, chat: &str, call: &str, server: &str) -> ToolOutcome {
    // Failure to locate storage must not discard the tool's text or change its status.
    let directory = artifact_directory();
    normalize_in(result, chat, call, server, directory.as_deref())
}

fn normalize_in(
    result: Value,
    chat: &str,
    call: &str,
    server: &str,
    directory: Result<&Path, &String>,
) -> ToolOutcome {
    let mut out = ToolOutcome {
        status: if result.get("isError").and_then(Value::as_bool) == Some(true) {
            ToolStatus::Failed
        } else {
            ToolStatus::Success
        },
        ..Default::default()
    };
    let mut text = Vec::new();
    let mut total = 0;
    if let Some(content) = result.get("content").and_then(Value::as_array) {
        for block in content {
            let kind = block
                .get("type")
                .and_then(Value::as_str)
                .unwrap_or("unknown");
            if kind == "text" {
                if let Some(t) = block.get("text").and_then(Value::as_str) {
                    text.push(t.to_owned());
                }
                continue;
            }
            if kind == "resource_link" {
                if let Some(uri) = block.get("uri").and_then(Value::as_str) {
                    let name = block.get("name").and_then(Value::as_str).unwrap_or(uri);
                    text.push(format!("Resource: {name} ({uri})"));
                    out.artifacts.push(McpArtifact {
                        id: uuid::Uuid::new_v4().to_string(),
                        name: name.into(),
                        mime_type: block
                            .get("mimeType")
                            .and_then(Value::as_str)
                            .unwrap_or("application/octet-stream")
                            .into(),
                        kind: "link".into(),
                        size: 0,
                        chat_id: chat.into(),
                        call_id: call.into(),
                        server: server.into(),
                        uri: Some(uri.into()),
                    });
                }
                continue;
            }
            let resource = if kind == "resource" {
                block.get("resource").unwrap_or(&Value::Null)
            } else {
                block
            };
            let mime = resource
                .get("mimeType")
                .and_then(Value::as_str)
                .unwrap_or("application/octet-stream");
            let is_image = kind == "image";
            let data = resource
                .get(if is_image { "data" } else { "blob" })
                .and_then(Value::as_str);
            let bytes = if let Some(encoded) = data {
                if encoded.len() > ITEM_LIMIT.div_ceil(3) * 4 + 4 {
                    out.warnings.push("Artifact exceeds 20 MiB".into());
                    continue;
                }
                match STANDARD.decode(encoded) {
                    Ok(bytes) => bytes,
                    Err(_) => {
                        out.warnings.push("Invalid artifact base64".into());
                        continue;
                    }
                }
            } else if kind == "resource" {
                if let Some(t) = resource.get("text").and_then(Value::as_str) {
                    t.as_bytes().to_vec()
                } else {
                    text.push("Embedded resource has no supported payload".into());
                    continue;
                }
            } else {
                text.push(format!("Unsupported MCP content: {kind}"));
                continue;
            };
            if bytes.len() > ITEM_LIMIT || total + bytes.len() > RESULT_LIMIT {
                out.warnings
                    .push("Artifact size limit exceeded (20 MiB/item, 50 MiB/result)".into());
                continue;
            }
            total += bytes.len();
            if is_image {
                let format = match mime {
                    "image/png" => Some(image::ImageFormat::Png),
                    "image/jpeg" => Some(image::ImageFormat::Jpeg),
                    "image/webp" => Some(image::ImageFormat::WebP),
                    "image/gif" => Some(image::ImageFormat::Gif),
                    _ => None,
                };
                let Some(format) = format else {
                    out.warnings.push(format!("Unsupported image type: {mime}"));
                    continue;
                };
                let mut reader =
                    image::ImageReader::with_format(std::io::Cursor::new(&bytes), format);
                reader.limits(image::Limits::default());
                if reader.decode().is_err() {
                    out.warnings.push("Invalid image payload".into());
                    continue;
                }
                out.images
                    .push(format!("data:{mime};base64,{}", STANDARD.encode(&bytes)));
            }
            let name = resource
                .get("name")
                .or_else(|| resource.get("uri"))
                .and_then(Value::as_str)
                .unwrap_or(if is_image {
                    "MCP image"
                } else {
                    "MCP attachment"
                });
            let mut entry = McpArtifact {
                id: uuid::Uuid::new_v4().to_string(),
                name: name.rsplit('/').next().unwrap_or(name).into(),
                mime_type: mime.into(),
                kind: if is_image { "image" } else { "file" }.into(),
                size: bytes.len(),
                chat_id: chat.into(),
                call_id: call.into(),
                server: server.into(),
                uri: None,
            };
            if entry.name.is_empty() {
                entry.name = "MCP attachment".into();
            }
            let saved = directory.map_err(Clone::clone).and_then(|dir| {
                fs::create_dir_all(dir).map_err(|e| e.to_string())?;
                fs::write(dir.join(&entry.id), &bytes).map_err(|e| e.to_string())?;
                fs::write(
                    dir.join(format!("{}.json", entry.id)),
                    serde_json::to_vec(&entry).map_err(|e| e.to_string())?,
                )
                .map_err(|e| e.to_string())
            });
            match saved {
                Ok(()) => {
                    text.push(format!(
                        "Attached {}: {} (artifact {})",
                        entry.kind, entry.name, entry.id
                    ));
                    out.artifacts.push(entry);
                }
                Err(e) => out
                    .warnings
                    .push(format!("Could not save {}: {e}", entry.name)),
            }
        }
    } else {
        text.push(serde_json::to_string_pretty(&result).unwrap_or_default());
    }
    if let Some(structured) = result.get("structuredContent") {
        text.push(format!(
            "Structured result: {}",
            serde_json::to_string(structured).unwrap_or_default()
        ));
    }
    text.extend(out.warnings.iter().map(|w| format!("Warning: {w}")));
    out.text = text.join("\n");
    if out.status == ToolStatus::Failed {
        out.text = format!("Error: MCP tool reported failure\n{}", out.text);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    fn directory() -> PathBuf {
        std::env::temp_dir().join(format!("mint-artifact-{}", uuid::Uuid::new_v4()))
    }
    fn png() -> String {
        let mut bytes = std::io::Cursor::new(Vec::new());
        image::DynamicImage::new_rgb8(1, 1)
            .write_to(&mut bytes, image::ImageFormat::Png)
            .unwrap();
        STANDARD.encode(bytes.into_inner())
    }
    #[test]
    fn mixed_result_preserves_images_files_and_links_after_reload() {
        let dir = directory();
        let encoded = png();
        let out = normalize_in(
            json!({"content":[{"type":"text","text":"done"},{"type":"image","mimeType":"image/png","data":encoded},{"type":"image","mimeType":"image/png","data":png()},{"type":"resource","resource":{"uri":"file:///model.obj","mimeType":"text/plain","text":"v 1 2 3"}},{"type":"resource_link","uri":"https://example.test/model","name":"model"}],"structuredContent":{"objects":1}}),
            "chat",
            "call",
            "blender",
            Ok(&dir),
        );
        assert_eq!(out.status, ToolStatus::Success);
        assert_eq!(out.images.len(), 2);
        assert_eq!(out.artifacts.len(), 4);
        assert!(!out.text.contains(&encoded));
        assert!(out.text.contains("objects"));
        let (entry, bytes) = read_artifact_in(&dir, &out.artifacts[2].id).unwrap();
        assert_eq!(entry.chat_id, "chat");
        assert_eq!(bytes, b"v 1 2 3");
        assert!(read_artifact_in(&dir, "../../etc/passwd").is_err());
        fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    fn failed_tool_keeps_diagnostics_and_artifacts() {
        let dir = directory();
        let out = normalize_in(
            json!({"isError":true,"content":[{"type":"text","text":"render failed"},{"type":"image","mimeType":"image/png","data":png()}]}),
            "c",
            "r",
            "s",
            Ok(&dir),
        );
        assert_eq!(out.status, ToolStatus::Failed);
        assert!(out.text.contains("render failed"));
        assert_eq!(out.artifacts.len(), 1);
        fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    fn invalid_payload_and_storage_failure_do_not_discard_text_or_tool_status() {
        let dir = directory();
        fs::write(&dir, b"not a directory").unwrap();
        let out = normalize_in(
            json!({"content":[{"type":"text","text":"created"},{"type":"image","mimeType":"image/png","data":"!!!!"},{"type":"image","mimeType":"image/png","data":png()}]}),
            "c",
            "r",
            "s",
            Ok(&dir),
        );
        assert_eq!(out.status, ToolStatus::Success);
        assert!(out.text.contains("created"));
        assert_eq!(out.warnings.len(), 2);
        assert!(out.artifacts.is_empty());
        assert_eq!(out.images.len(), 1);
        fs::remove_file(dir).unwrap();
    }
    #[test]
    fn rejects_oversized_artifact_without_decoding_it() {
        let out = normalize_in(
            json!({"content":[{"type":"image","mimeType":"image/png","data":"A".repeat(ITEM_LIMIT.div_ceil(3)*4+5)}]}),
            "c",
            "r",
            "s",
            Ok(Path::new("/unused")),
        );
        assert!(out.images.is_empty());
        assert!(out.warnings[0].contains("20 MiB"));
    }
}

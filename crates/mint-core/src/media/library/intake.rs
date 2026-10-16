use super::*;

#[derive(Debug, Default, Clone, Deserialize, Serialize)]
#[serde(default, rename_all = "camelCase")]
pub struct AttachmentMetadata {
    pub request_id: String,
    pub image_names: Vec<String>,
    pub video_names: Vec<String>,
}
#[derive(Clone)]
pub struct AttachmentReceipt {
    store: LibraryStore,
    chat_id: String,
    request_id: String,
}
impl AttachmentReceipt {
    pub fn bind(&self, id: i64) -> Result<()> {
        self.store.bind_request(&self.chat_id, &self.request_id, id)
    }
}
impl LibraryStore {
    /// Persist originals before extraction or any AI/network work. Failed AI runs keep their receipts.
    pub fn prepare(
        &self,
        request: &crate::ChatRequest,
        metadata: Option<&AttachmentMetadata>,
        config: &crate::MintConfig,
    ) -> std::result::Result<(crate::ChatRequest, Option<AttachmentReceipt>), String> {
        let mut files = vec![];
        for (kind, uris, names) in [
            (
                "Image",
                request.image_data_uri.as_deref(),
                metadata.map(|m| &m.image_names),
            ),
            (
                "Video",
                request.video_data_uri.as_deref(),
                metadata.map(|m| &m.video_names),
            ),
        ] {
            let uris: Vec<_> = uris.unwrap_or("").split_whitespace().collect();
            if let Some(names) = names
                && names.len() != uris.len()
            {
                return Err("Attachment names do not match the submitted files".into());
            }
            for (index, uri) in uris.iter().enumerate() {
                let extension = uri
                    .strip_prefix("data:")
                    .and_then(|s| s.split_once(';'))
                    .and_then(|(mime, _)| mime.split_once('/'))
                    .map(|(_, ext)| ext)
                    .unwrap_or("bin");
                let name = names
                    .map(|n| n[index].clone())
                    .unwrap_or_else(|| format!("{kind} {}.{extension}", index + 1));
                files.push(LibraryUpload {
                    name,
                    data_uri: (*uri).into(),
                });
            }
        }
        if let Some(document) = &request.document_attachment {
            files.push(LibraryUpload {
                name: document.filename.clone(),
                data_uri: document.data_uri.clone(),
            });
        }
        if files.is_empty() {
            return Ok((request.clone(), None));
        }
        let chat_id = request
            .chat_id
            .as_deref()
            .filter(|s| !s.trim().is_empty())
            .unwrap_or(crate::DEFAULT_CONVERSATION_ID)
            .trim()
            .to_owned();
        let request_id = metadata
            .map(|m| m.request_id.clone())
            .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
        let items = self
            .import(
                &files,
                &request_id,
                Some(&chat_id),
                "uploaded",
                "chat",
                &request.message,
            )
            .map_err(|e| e.to_string())?;
        let receipt = AttachmentReceipt {
            store: self.clone(),
            chat_id,
            request_id,
        };
        let enriched = if request.document_attachment.is_some() {
            request.with_stored_document_context(
                config,
                &self
                    .managed_path(&items.last().unwrap().path)
                    .map_err(|e| e.to_string())?,
            )?
        } else {
            request.clone()
        };
        Ok((enriched, Some(receipt)))
    }
}

/// Isolate disk IO and document extraction from the async request executor.
pub async fn prepare_chat(
    request: &crate::ChatRequest,
    metadata: Option<&AttachmentMetadata>,
    config: &crate::MintConfig,
) -> std::result::Result<(crate::ChatRequest, Option<AttachmentReceipt>), String> {
    if request
        .image_data_uri
        .as_deref()
        .unwrap_or("")
        .trim()
        .is_empty()
        && request
            .video_data_uri
            .as_deref()
            .unwrap_or("")
            .trim()
            .is_empty()
        && request.document_attachment.is_none()
    {
        return Ok((request.clone(), None));
    }
    let request = request.clone();
    let metadata = metadata.cloned();
    let config = config.clone();
    tokio::task::spawn_blocking(move || {
        LibraryStore::open_default()
            .map_err(|e| e.to_string())?
            .prepare(&request, metadata.as_ref(), &config)
    })
    .await
    .map_err(|e| e.to_string())?
}

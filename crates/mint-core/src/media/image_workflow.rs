//! Generate images and persist them in Library before reporting success.

use crate::{
    MintConfig,
    image_gen::{ImageGenError, ImageGenRequest, ImageGenResponse},
    pictures::{PictureEntry, PictureError},
};
use serde::Serialize;
use std::future::Future;
use thiserror::Error;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SavedImageGenResponse {
    pub images: Vec<PictureEntry>,
    pub model: String,
    pub provider: String,
    pub prompt: String,
    pub description: Option<String>,
}

#[derive(Debug, Error)]
pub enum ImageWorkflowError {
    #[error(transparent)]
    Generate(#[from] ImageGenError),
    #[error(transparent)]
    Save(#[from] PictureError),
}

/// Returns persisted Library media entries, preserving the actual generation metadata.
/// Provider or persistence errors propagate; invalid image data is rejected before writes.
pub async fn generate_and_save_images(
    config: &MintConfig,
    request: &ImageGenRequest,
) -> Result<SavedImageGenResponse, ImageWorkflowError> {
    generate_and_save_with(
        || crate::image_gen::generate_images(config, request),
        |images, source, prompt| {
            crate::save_chat_images(images, Some(source.into()), Some(prompt.into()))
        },
    )
    .await
}

async fn generate_and_save_with<G, Fut, S>(
    generate: G,
    save: S,
) -> Result<SavedImageGenResponse, ImageWorkflowError>
where
    G: FnOnce() -> Fut,
    Fut: Future<Output = Result<ImageGenResponse, ImageGenError>>,
    S: FnOnce(Vec<String>, &str, &str) -> Result<Vec<PictureEntry>, PictureError>,
{
    let result = generate().await?;
    if result.images.is_empty() {
        return Err(ImageGenError::NoImagesReturned.into());
    }
    for image in &result.images {
        if !crate::parse_data_uri(&image.data_uri)
            .is_some_and(|(mime, _, bytes)| mime.starts_with("image/") && !bytes.is_empty())
        {
            return Err(PictureError::InvalidDataUri.into());
        }
    }
    let images = save(
        result
            .images
            .into_iter()
            .map(|image| image.data_uri)
            .collect(),
        &result.provider,
        &result.prompt,
    )?;
    Ok(SavedImageGenResponse {
        images,
        model: result.model,
        provider: result.provider,
        prompt: result.prompt,
        description: result.description,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::image_gen::GeneratedImage;
    use std::{fs, path::PathBuf};

    struct Fixture(PathBuf);
    impl Fixture {
        fn store(&self) -> Result<crate::library::LibraryStore, crate::library::LibraryError> {
            crate::library::LibraryStore::open(
                crate::MemoryStore::open(self.0.join("memory.sqlite")),
                self.0.join("Library"),
                self.0.join("Pictures"),
            )
        }
        fn save(
            &self,
            images: Vec<String>,
            source: &str,
            message: &str,
        ) -> Result<Vec<PictureEntry>, PictureError> {
            crate::pictures::save_chat_images_in(
                self.store()?,
                images,
                Some(source.into()),
                Some(message.into()),
            )
        }
        fn new() -> Self {
            Self(std::env::temp_dir().join(format!("mint-image-workflow-{}", uuid::Uuid::new_v4())))
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }
    fn generated(images: &[&str]) -> ImageGenResponse {
        ImageGenResponse {
            images: images
                .iter()
                .map(|data| GeneratedImage {
                    data_uri: (*data).into(),
                    mime_type: "image/png".into(),
                })
                .collect(),
            model: "actual-model".into(),
            provider: "replicate".into(),
            prompt: "provider prompt".into(),
            description: Some("description".into()),
        }
    }

    #[tokio::test]
    async fn core_migration_saved_images_use_actual_provider_and_return_persisted_metadata() {
        let f = Fixture::new();
        let result = generate_and_save_with(
            || async {
                Ok(generated(&[
                    "data:image/png;base64,aGk=",
                    "data:image/png;base64,dGhlcmU=",
                ]))
            },
            |images, source, message| f.save(images, source, message),
        )
        .await
        .unwrap();
        assert_eq!(result.images.len(), 2);
        assert_eq!(fs::read(&result.images[0].path).unwrap(), b"hi");
        assert_eq!(fs::read(&result.images[1].path).unwrap(), b"there");
        assert_eq!(result.images[0].source, "replicate");
        assert_eq!(result.images[0].message, "provider prompt");
        assert_eq!(result.model, "actual-model");
        assert_eq!(result.provider, "replicate");
        assert_eq!(result.description.as_deref(), Some("description"));
        let index = f
            .store()
            .unwrap()
            .list(&crate::library::LibraryQuery::default())
            .unwrap();
        assert_eq!(index.items.len(), 2);
        assert!(index.items.iter().all(|item| item.origin == "generated"));
        assert!(
            index
                .items
                .iter()
                .any(|item| item.id == result.images[1].id)
        );
        assert!(!f.0.join("Pictures/pictures.json").exists());
    }

    #[tokio::test]
    async fn core_migration_failed_generation_does_not_write_pictures() {
        let f = Fixture::new();
        let result = generate_and_save_with(
            || async { Err(ImageGenError::ModelError("provider unavailable".into())) },
            |images, source, message| f.save(images, source, message),
        )
        .await;
        assert!(matches!(
            result,
            Err(ImageWorkflowError::Generate(ImageGenError::ModelError(_)))
        ));
        assert!(!f.0.exists());
    }

    #[tokio::test]
    async fn core_migration_failed_persistence_is_not_reported_as_success() {
        let f = Fixture::new();
        fs::write(&f.0, "a file blocks the Pictures directory").unwrap();
        let result = generate_and_save_with(
            || async { Ok(generated(&["data:image/png;base64,aGk="])) },
            |images, source, message| f.save(images, source, message),
        )
        .await;
        assert!(matches!(result, Err(ImageWorkflowError::Save(_))));
        let _ = fs::remove_file(&f.0);
    }

    #[tokio::test]
    async fn core_migration_invalid_generated_image_is_rejected_before_partial_persistence() {
        let f = Fixture::new();
        let result = generate_and_save_with(
            || async { Ok(generated(&["data:image/png;base64,aGk=", "invalid image"])) },
            |images, source, message| f.save(images, source, message),
        )
        .await;
        assert!(matches!(
            result,
            Err(ImageWorkflowError::Save(PictureError::InvalidDataUri))
        ));
        assert!(!f.0.exists());
    }
}

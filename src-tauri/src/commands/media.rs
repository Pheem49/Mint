use mint_core::{
    ImageGenRequest, PictureEntry, TtsUrl, VideoGenRequest, VideoGenResponse, WeatherReport,
    delete_saved_picture, google_tts_urls, list_saved_pictures, load_config, save_chat_images,
    weather,
};
pub(crate) type DesktopImageGenResponse = mint_core::image_workflow::SavedImageGenResponse;

/// Returns discovered image models followed by missing presets.
#[tauri::command]
pub(crate) async fn fetch_image_provider_models(
    provider: String,
    api_key: String,
) -> Result<Vec<String>, String> {
    let config = load_config().map_err(|e| e.to_string())?;
    Ok(mint_core::model_catalog::image_models(&config, &provider, &api_key).await)
}

/// Returns discovered video models followed by missing presets.
#[tauri::command]
pub(crate) async fn fetch_video_provider_models(
    provider: String,
    api_key: String,
) -> Result<Vec<String>, String> {
    let config = load_config().map_err(|e| e.to_string())?;
    Ok(mint_core::model_catalog::video_models(&config, &provider, &api_key).await)
}

/// Returns discovered Gemini Live models followed by missing presets.
#[tauri::command]
pub(crate) async fn fetch_gemini_live_models(api_key: String) -> Result<Vec<String>, String> {
    let config = load_config().map_err(|e| e.to_string())?;
    Ok(mint_core::model_catalog::live_models(&config, &api_key).await)
}

#[tauri::command]
pub(crate) async fn generate_images(
    request: ImageGenRequest,
) -> Result<DesktopImageGenResponse, String> {
    let config = load_config().map_err(|error| error.to_string())?;

    mint_core::image_workflow::generate_and_save_images(&config, &request)
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub(crate) async fn generate_video(request: VideoGenRequest) -> Result<VideoGenResponse, String> {
    let config = load_config().map_err(|error| error.to_string())?;
    mint_core::generate_video(&config, &request)
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub(crate) fn list_pictures() -> Result<Vec<PictureEntry>, String> {
    list_saved_pictures().map_err(|error| error.to_string())
}

#[tauri::command]
pub(crate) fn delete_picture(id: String) -> Result<(), String> {
    delete_saved_picture(&id).map_err(|error| error.to_string())
}

#[tauri::command]
pub(crate) fn save_pictures(
    images: Vec<String>,
    source: Option<String>,
    message: Option<String>,
) -> Result<Vec<PictureEntry>, String> {
    save_chat_images(images, source, message).map_err(|error| error.to_string())
}

#[tauri::command]
pub(crate) fn upload_file(filename: String, data_b64: String) -> Result<String, String> {
    // Determine mime from filename
    let mime = filename
        .rsplit_once('.')
        .map(|(_, ext)| match ext.to_ascii_lowercase().as_str() {
            "mp4" => "video/mp4",
            "webm" => "video/webm",
            "mov" => "video/quicktime",
            "mkv" => "video/x-matroska",
            "avi" => "video/x-msvideo",
            _ => "application/octet-stream",
        })
        .unwrap_or("application/octet-stream");

    let data_uri = format!("data:{};base64,{}", mime, data_b64);
    match save_chat_images(
        vec![data_uri],
        Some("upload".into()),
        Some("uploaded".into()),
    ) {
        Ok(mut saved) => {
            if let Some(entry) = saved.pop() {
                return Ok(format!("/api/pictures/{}", entry.filename));
            }
            Err("no entry saved".into())
        }
        Err(e) => Err(e.to_string()),
    }
}

#[tauri::command]
pub(crate) fn get_tts_urls(text: String) -> Result<Vec<TtsUrl>, String> {
    let language = load_config().map_err(|error| error.to_string())?.language;
    Ok(google_tts_urls(&text, &language))
}

#[tauri::command]
pub(crate) async fn get_weather(city: String) -> Result<WeatherReport, String> {
    weather(&city).await.map_err(|error| error.to_string())
}

#[tauri::command]
pub(crate) async fn list_library(
    query: mint_core::library::LibraryQuery,
) -> Result<mint_core::library::LibraryPage, String> {
    tokio::task::spawn_blocking(move || {
        mint_core::library::LibraryStore::open_default()
            .and_then(|s| s.list(&query))
            .map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| e.to_string())?
}
#[tauri::command]
pub(crate) async fn get_library_item(
    id: String,
) -> Result<mint_core::library::LibraryItem, String> {
    tokio::task::spawn_blocking(move || {
        mint_core::library::LibraryStore::open_default()
            .and_then(|s| s.get(&id))
            .map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| e.to_string())?
}
#[tauri::command]
pub(crate) async fn set_library_trashed(id: String, trashed: bool) -> Result<(), String> {
    tokio::task::spawn_blocking(move || {
        mint_core::library::LibraryStore::open_default()
            .and_then(|s| s.set_trashed(&id, trashed))
            .map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| e.to_string())?
}
#[tauri::command]
pub(crate) async fn read_library_file(id: String, thumbnail: bool) -> Result<String, String> {
    tokio::task::spawn_blocking(move || {
        use base64::Engine;
        let (item, bytes) = mint_core::library::LibraryStore::open_default()
            .and_then(|s| s.read(&id, thumbnail))
            .map_err(|e| e.to_string())?;
        Ok(format!(
            "data:{};base64,{}",
            if thumbnail {
                "image/png"
            } else {
                &item.mime_type
            },
            base64::engine::general_purpose::STANDARD.encode(bytes)
        ))
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
pub(crate) async fn remove_library_item(id: String) -> Result<(), String> {
    tokio::task::spawn_blocking(move || {
        mint_core::library::LibraryStore::open_default()
            .and_then(|s| s.remove(&id))
            .map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
pub(crate) async fn speech_request(
    request: mint_core::tts::playback::SpeechRequest,
) -> Result<mint_core::tts::playback::SpeechReply, String> {
    mint_core::tts::playback::request(request).await
}

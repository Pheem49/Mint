use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
use mint_core::{
    AuthUser, MemoryStore, config_path, get_user, load_config, login_user, register_user,
    save_avatar_file, update_profile,
};
use serde_json::Value;
use std::fs;
use std::path::PathBuf;

#[tauri::command]
pub(crate) fn get_profile_value(key: String) -> Result<Option<String>, String> {
    if let Ok(memory) = MemoryStore::open_default() {
        if let Ok(Some(val)) = memory.get_profile(&key) {
            if !val.trim().is_empty() {
                return Ok(Some(val));
            }
        }
    }
    if let Ok(cfg) = load_config() {
        if key == "veoModel" {
            let m = mint_core::media::video_models::active_video_model_for_provider(&cfg, "veo");
            if !m.is_empty() {
                return Ok(Some(m.to_string()));
            }
        } else if key == "videoGenProvider" {
            let p = cfg
                .extra
                .get("videoGenProvider")
                .and_then(|v| v.as_str())
                .unwrap_or("veo");
            return Ok(Some(p.to_string()));
        }
    }
    Ok(None)
}

#[tauri::command]
pub(crate) fn set_profile_value(key: String, value: String) -> Result<(), String> {
    MemoryStore::open_default()
        .and_then(|memory| memory.set_profile(&key, &value))
        .map_err(|error| error.to_string())
}

/// Path to the file that remembers which shared-store user is currently
/// logged into the desktop app (`~/.config/mint/session.json`). Desktop is a
/// single trusted process per launch, so a bearer token isn't needed here —
/// unlike the web-mode API server (see mint_core::auth session tokens).
fn desktop_session_path() -> Result<PathBuf, String> {
    Ok(config_path()
        .map_err(|error| error.to_string())?
        .with_file_name("session.json"))
}

fn write_desktop_session(user_id: &str) -> Result<(), String> {
    let path = desktop_session_path()?;
    let contents = serde_json::json!({ "userId": user_id }).to_string();
    fs::write(path, contents).map_err(|error| error.to_string())
}

fn read_desktop_session_user_id() -> Option<String> {
    let path = desktop_session_path().ok()?;
    let contents = fs::read_to_string(path).ok()?;
    let value: Value = serde_json::from_str(&contents).ok()?;
    value.get("userId")?.as_str().map(str::to_string)
}

#[tauri::command]
pub(crate) fn auth_register(
    name: Option<String>,
    email: String,
    password: String,
) -> Result<AuthUser, String> {
    let user = register_user(name, &email, &password).map_err(|error| error.to_string())?;
    write_desktop_session(&user.id)?;
    Ok(user)
}

#[tauri::command]
pub(crate) fn auth_login(email: String, password: String) -> Result<AuthUser, String> {
    let user = login_user(&email, &password).map_err(|error| error.to_string())?;
    write_desktop_session(&user.id)?;
    Ok(user)
}

#[tauri::command]
pub(crate) fn auth_logout() -> Result<(), String> {
    let path = desktop_session_path()?;
    if path.exists() {
        fs::remove_file(path).map_err(|error| error.to_string())?;
    }
    Ok(())
}

#[tauri::command]
pub(crate) fn auth_current_user() -> Result<Option<AuthUser>, String> {
    let Some(user_id) = read_desktop_session_user_id() else {
        return Ok(None);
    };
    get_user(&user_id).map_err(|error| error.to_string())
}

#[tauri::command]
pub(crate) fn auth_update_profile(
    name: Option<String>,
    image: Option<String>,
) -> Result<AuthUser, String> {
    let Some(user_id) = read_desktop_session_user_id() else {
        return Err("Not logged in".to_string());
    };
    update_profile(&user_id, name, image).map_err(|error| error.to_string())
}

#[tauri::command]
pub(crate) fn auth_upload_avatar(
    file_name: String,
    data_base64: String,
) -> Result<AuthUser, String> {
    let Some(user_id) = read_desktop_session_user_id() else {
        return Err("Not logged in".to_string());
    };
    let bytes = BASE64
        .decode(data_base64.as_bytes())
        .map_err(|_| "Invalid image data".to_string())?;
    let extension = file_name.rsplit('.').next().unwrap_or("png").to_lowercase();
    let url = save_avatar_file(&bytes, &extension).map_err(|error| error.to_string())?;
    update_profile(&user_id, None, Some(url)).map_err(|error| error.to_string())
}

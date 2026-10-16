use mint_core::browser::{
    BrowserTab, click as browser_click, list_tabs as browser_list_tabs,
    navigate as browser_navigate, read_page_text,
};
use mint_core::load_config;
use std::sync::{LazyLock, Mutex};
use tauri::{AppHandle, Emitter, Manager};

#[allow(dead_code)]
const MINT_BROWSER_SHELL_HEIGHT: u32 = 92;
const MINT_BROWSER_SUGGESTIONS_HEIGHT: u32 = 164;
static MINT_BROWSER_SUGGESTION_QUERY: LazyLock<Mutex<String>> =
    LazyLock::new(|| Mutex::new(String::new()));

fn position_mint_browser_suggestions(app: &AppHandle) -> Result<(), String> {
    let browser = app
        .get_window("mint-browser")
        .ok_or("Mint Browser is not open")?;
    let popup = app
        .get_webview_window("mint-browser-suggestions")
        .ok_or("Mint Browser suggestions are not open")?;
    let position = browser
        .inner_position()
        .map_err(|error| error.to_string())?;
    let size = browser.inner_size().map_err(|error| error.to_string())?;
    let popup_width = size.width.saturating_sub(164).max(320);
    popup
        .set_position(tauri::PhysicalPosition::new(
            position.x + 114,
            position.y + 86,
        ))
        .map_err(|error| error.to_string())?;
    popup
        .set_size(tauri::PhysicalSize::new(
            popup_width,
            MINT_BROWSER_SUGGESTIONS_HEIGHT,
        ))
        .map_err(|error| error.to_string())
}

#[allow(dead_code)]
fn position_mint_browser_shell(app: &AppHandle) -> Result<(), String> {
    let browser = app
        .get_window("mint-browser")
        .ok_or("Mint Browser is not open")?;
    let shell = app
        .get_window("mint-browser-shell-window")
        .ok_or("Mint Browser toolbar is not open")?;
    let position = browser
        .inner_position()
        .map_err(|error| error.to_string())?;
    let size = browser.inner_size().map_err(|error| error.to_string())?;
    shell
        .set_position(tauri::PhysicalPosition::new(position.x, position.y))
        .map_err(|error| error.to_string())?;
    shell
        .set_size(tauri::PhysicalSize::new(
            size.width,
            MINT_BROWSER_SHELL_HEIGHT,
        ))
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub(crate) async fn open_mint_browser(app: AppHandle, url: String) -> Result<(), String> {
    let target_url = if url.trim().is_empty() {
        "https://www.google.com".to_string()
    } else {
        let trimmed = url.trim();
        if trimmed.starts_with("http://") || trimmed.starts_with("https://") {
            trimmed.to_string()
        } else {
            format!("https://{trimmed}")
        }
    };

    let mut config = load_config().map_err(|e| e.to_string())?;
    if mint_core::enable_browser_tools(&mut config) {
        let _ = mint_core::save_config(&config);
    }

    mint_core::spawn_automation_browser_with_url(&config, Some(&target_url)).await?;
    let _ = app.emit("mint-browser-address", target_url.as_str());
    Ok(())
}

#[tauri::command]
pub(crate) fn close_mint_browser(app: AppHandle) -> Result<(), String> {
    if let Some(popup) = app.get_webview_window("mint-browser-suggestions") {
        let _ = popup.close();
    }
    if let Some(shell) = app.get_window("mint-browser-shell-window") {
        let _ = shell.close();
    }
    if let Some(browser) = app.get_window("mint-browser") {
        browser.close().map_err(|error| error.to_string())?;
    }
    Ok(())
}

#[tauri::command]
pub(crate) fn show_mint_browser_suggestions(app: AppHandle, query: String) -> Result<(), String> {
    *MINT_BROWSER_SUGGESTION_QUERY
        .lock()
        .map_err(|_| "Suggestion query is unavailable".to_string())? = query.clone();

    let popup = if let Some(popup) = app.get_webview_window("mint-browser-suggestions") {
        popup
    } else {
        tauri::webview::WebviewWindowBuilder::new(
            &app,
            "mint-browser-suggestions",
            tauri::WebviewUrl::App("index.html?mint-browser-suggestions=1".into()),
        )
        .title("Mint Browser Suggestions")
        .decorations(false)
        .transparent(true)
        .always_on_top(true)
        .skip_taskbar(true)
        .resizable(false)
        .focused(false)
        .visible(false)
        .build()
        .map_err(|error| format!("Could not open browser suggestions: {error}"))?
    };

    position_mint_browser_suggestions(&app)?;
    let _ = app.emit_to(
        "mint-browser-suggestions",
        "mint-browser-suggestions-update",
        query,
    );
    popup.show().map_err(|error| error.to_string())
}

#[tauri::command]
pub(crate) fn hide_mint_browser_suggestions(app: AppHandle) -> Result<(), String> {
    if let Some(popup) = app.get_webview_window("mint-browser-suggestions") {
        popup.hide().map_err(|error| error.to_string())?;
    }
    Ok(())
}

#[tauri::command]
pub(crate) fn get_mint_browser_suggestion_query() -> Result<String, String> {
    MINT_BROWSER_SUGGESTION_QUERY
        .lock()
        .map(|query| query.clone())
        .map_err(|_| "Suggestion query is unavailable".to_string())
}

#[tauri::command]
pub(crate) async fn navigate_mint_browser(app: AppHandle, url: String) -> Result<(), String> {
    let target_url = if url.trim().is_empty() {
        "https://www.google.com".to_string()
    } else {
        let trimmed = url.trim();
        if trimmed.starts_with("http://") || trimmed.starts_with("https://") {
            trimmed.to_string()
        } else {
            format!("https://{trimmed}")
        }
    };

    if let Ok(config) = load_config() {
        if mint_core::is_browser_running(&config).await {
            let _ = mint_core::browser::ensure_page_open(&config).await;
            let _ = mint_core::browser::navigate(&config, &target_url).await;
        }
    }
    if let Some(page) = app.get_webview("mint-browser-page") {
        if let Ok(parsed) = tauri::Url::parse(&target_url) {
            let _ = page.navigate(parsed);
        }
    }
    let _ = app.emit("mint-browser-address", target_url.as_str());
    Ok(())
}

#[tauri::command]
pub(crate) fn browser_go_back(app: AppHandle) -> Result<(), String> {
    if let Some(page) = app.get_webview("mint-browser-page") {
        let _ = page.eval("history.back()");
    }
    Ok(())
}

#[tauri::command]
pub(crate) fn browser_go_forward(app: AppHandle) -> Result<(), String> {
    if let Some(page) = app.get_webview("mint-browser-page") {
        let _ = page.eval("history.forward()");
    }
    Ok(())
}

#[tauri::command]
pub(crate) fn browser_reload(app: AppHandle) -> Result<(), String> {
    if let Some(page) = app.get_webview("mint-browser-page") {
        let _ = page.reload();
    }
    Ok(())
}

#[tauri::command]
pub(crate) async fn get_browser_tabs() -> Result<Vec<BrowserTab>, String> {
    browser_list_tabs(&load_config().map_err(|error| error.to_string())?).await
}

#[tauri::command]
pub(crate) async fn navigate_browser(
    url: String,
    tab_id: Option<String>,
) -> Result<String, String> {
    let config = load_config().map_err(|error| error.to_string())?;
    mint_core::browser::with_tab(&config, tab_id.as_deref(), browser_navigate(&config, &url))
        .await?
}

#[tauri::command]
pub(crate) async fn read_browser_page(tab_id: Option<String>) -> Result<String, String> {
    let config = load_config().map_err(|error| error.to_string())?;
    mint_core::browser::with_tab(&config, tab_id.as_deref(), read_page_text(&config)).await?
}

#[tauri::command]
pub(crate) async fn click_browser_selector(
    selector: String,
    tab_id: Option<String>,
) -> Result<String, String> {
    let config = load_config().map_err(|error| error.to_string())?;
    mint_core::browser::with_tab(
        &config,
        tab_id.as_deref(),
        browser_click(&config, &selector),
    )
    .await?
}

#[tauri::command]
pub(crate) async fn type_in_browser(
    selector: String,
    text: String,
    tab_id: Option<String>,
) -> Result<String, String> {
    let config = load_config().map_err(|error| error.to_string())?;
    mint_core::browser::with_tab(
        &config,
        tab_id.as_deref(),
        mint_core::browser::type_text(&config, &selector, &text),
    )
    .await?
}

//! Starting, detecting, and preparing the automation browser instance itself
//! (as opposed to driving an already-running one — see `navigate`/`interact`).

use crate::MintConfig;
use serde_json::Value;

use super::cdp::fetch_pages_endpoint;

pub async fn is_browser_running(config: &MintConfig) -> bool {
    let endpoint = config
        .extra
        .get("browserDebugUrl")
        .and_then(Value::as_str)
        .unwrap_or("http://127.0.0.1:9222/json/list");
    fetch_pages_endpoint(endpoint).await.is_ok()
}

pub async fn spawn_automation_browser(config: &MintConfig) -> Result<(), String> {
    let endpoint = config
        .extra
        .get("browserDebugUrl")
        .and_then(Value::as_str)
        .unwrap_or("http://127.0.0.1:9222/json/list");

    if fetch_pages_endpoint(endpoint).await.is_ok() {
        return Ok(());
    }

    let browser_name = config
        .extra
        .get("automationBrowser")
        .and_then(Value::as_str)
        .unwrap_or("chromium");

    let profile_dir = dirs::config_dir()
        .unwrap_or_else(std::env::temp_dir)
        .join("mint")
        .join("automation-profile");
    let profile_arg = format!("--user-data-dir={}", profile_dir.to_string_lossy());

    let args = [
        "--remote-debugging-port=9222".to_owned(),
        "--no-first-run".to_owned(),
        "--no-default-browser-check".to_owned(),
        profile_arg,
    ];

    let mut spawned = false;
    let executables: Vec<&str> = if cfg!(target_os = "windows") {
        vec!["chrome.exe", "chromium.exe", "msedge.exe"]
    } else if cfg!(target_os = "macos") {
        vec![
            "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome",
            "/Applications/Chromium.app/Contents/MacOS/Chromium",
            "chromium",
            "google-chrome",
        ]
    } else {
        vec![
            "chromium",
            "google-chrome-stable",
            "google-chrome",
            "chrome",
            "chromium-browser",
        ]
    };

    for exe in executables {
        if std::process::Command::new(exe)
            .args(&args)
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
            .is_ok()
        {
            spawned = true;
            break;
        }
    }

    if !spawned {
        return Err(format!(
            "Could not find or spawn browser '{browser_name}' with remote debugging. \
             Please verify it is installed."
        ));
    }

    for _ in 0..20 {
        tokio::time::sleep(std::time::Duration::from_millis(250)).await;
        if fetch_pages_endpoint(endpoint).await.is_ok() {
            return Ok(());
        }
    }

    Err("Browser spawned but remote debugging port 9222 did not become available.".to_string())
}

pub async fn ensure_page_open(config: &MintConfig) -> Result<(), String> {
    if !is_browser_running(config).await {
        return Err("Browser automation is not running. Please run 'mint auto' first.".to_string());
    }
    let endpoint = config
        .extra
        .get("browserDebugUrl")
        .and_then(Value::as_str)
        .unwrap_or("http://127.0.0.1:9222/json/list");

    let pages = fetch_pages_endpoint(endpoint).await?;
    let has_page = pages.iter().any(|p| p["type"] == "page");
    if !has_page {
        let base_url = endpoint.replace("/json/list", "/json/new");
        let client = crate::HTTP_CLIENT.clone();
        let _ = client
            .put(&base_url)
            .send()
            .await
            .map_err(|e| format!("failed to open new tab: {e}"))?;
    }
    Ok(())
}

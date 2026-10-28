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

pub(crate) const BROWSER_TOOLS: &[&str] = &[
    "browser_open",
    "browser_click",
    "browser_type",
    "browser_read",
    "browser_mouse_move",
    "browser_mouse_click",
    "browser_key_press",
    "browser_screenshot",
    "browser_tabs",
    "browser_observe",
    "browser_fill",
    "browser_select",
    "browser_scroll",
    "browser_wait",
];

pub fn enable_browser_tools(config: &mut MintConfig) -> bool {
    let mut changed = false;
    for tool in BROWSER_TOOLS {
        if config.disabled_tools.contains(&tool.to_string()) {
            config.disabled_tools.retain(|x| x != *tool);
            changed = true;
        }
    }
    changed
}

async fn spawn_browser_process(
    config: &MintConfig,
    initial_url: Option<&str>,
) -> Result<(), String> {
    let endpoint = config
        .extra
        .get("browserDebugUrl")
        .and_then(Value::as_str)
        .unwrap_or("http://127.0.0.1:9222/json/list");

    if fetch_pages_endpoint(endpoint).await.is_ok() {
        return Ok(());
    }

    let debug_url = reqwest::Url::parse(endpoint).map_err(|e| e.to_string())?;
    if !matches!(
        debug_url.host_str(),
        Some("localhost" | "127.0.0.1" | "[::1]" | "::1")
    ) {
        return Err("Configured remote browser is unavailable; start it at the configured endpoint before using Mint Auto".into());
    }
    let browser_name = config
        .extra
        .get("automationBrowser")
        .and_then(Value::as_str)
        .unwrap_or("chromium");

    let args = build_browser_args(config, endpoint, initial_url)?;

    let custom_exe = config
        .extra
        .get("browserExecutable")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty());

    let mut spawned = false;
    let mut executables: Vec<&str> = Vec::new();
    if let Some(exe) = custom_exe {
        executables.push(exe);
    }
    if cfg!(target_os = "windows") {
        executables.extend_from_slice(&["chrome.exe", "chromium.exe", "msedge.exe"]);
    } else if cfg!(target_os = "macos") {
        executables.extend_from_slice(&[
            "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome",
            "/Applications/Chromium.app/Contents/MacOS/Chromium",
            "chromium",
            "google-chrome",
        ]);
    } else {
        executables.extend_from_slice(&[
            "google-chrome",
            "google-chrome-stable",
            "chromium",
            "chrome",
            "chromium-browser",
            "/opt/google/chrome/google-chrome",
            "/opt/google/chrome/chrome",
        ]);
    }

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

    Err(format!(
        "Browser spawned but debugging endpoint {endpoint} did not become available."
    ))
}

pub async fn spawn_automation_browser(config: &MintConfig) -> Result<(), String> {
    spawn_browser_process(config, None).await
}

pub async fn ensure_page_open(config: &MintConfig) -> Result<(), String> {
    if !is_browser_running(config).await {
        spawn_browser_process(config, None).await?;
    }
    let endpoint = config
        .extra
        .get("browserDebugUrl")
        .and_then(Value::as_str)
        .unwrap_or("http://127.0.0.1:9222/json/list");

    let pages = fetch_pages_endpoint(endpoint).await?;
    if let Some(id) = super::selected_tab() {
        return if pages.iter().any(|p| p["type"] == "page" && p["id"] == id) {
            Ok(())
        } else {
            Err("tab_unavailable".into())
        };
    }
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

pub async fn spawn_automation_browser_with_url(
    config: &MintConfig,
    initial_url: Option<&str>,
) -> Result<(), String> {
    spawn_browser_process(config, initial_url).await?;
    if let Some(url) = initial_url {
        let trimmed = url.trim();
        if !trimmed.is_empty() {
            ensure_page_open(config).await?;
            super::navigate::navigate(config, trimmed).await?;
        }
    }
    Ok(())
}

pub(crate) fn build_browser_args(
    config: &MintConfig,
    endpoint: &str,
    initial_url: Option<&str>,
) -> Result<Vec<String>, String> {
    let agent_mode = config
        .extra
        .get("browserAgentMode")
        .and_then(Value::as_str)
        .unwrap_or("headless");

    let profile_dir = dirs::config_dir()
        .unwrap_or_else(std::env::temp_dir)
        .join("mint")
        .join("automation-profile");
    let profile_arg = format!("--user-data-dir={}", profile_dir.to_string_lossy());

    let mut args = vec![
        format!(
            "--remote-debugging-port={}",
            reqwest::Url::parse(endpoint)
                .map_err(|e| e.to_string())?
                .port_or_known_default()
                .ok_or("Missing debugging port")?
        ),
        "--no-first-run".to_owned(),
        "--no-default-browser-check".to_owned(),
        profile_arg,
    ];

    if agent_mode == "headless" {
        args.push("--headless=new".to_owned());
        args.push("--disable-gpu".to_owned());
        if cfg!(target_os = "linux") {
            args.push("--no-sandbox".to_owned());
            args.push("--disable-setuid-sandbox".to_owned());
            args.push("--disable-dev-shm-usage".to_owned());
        }
    }

    let width = config
        .extra
        .get("browserViewportWidth")
        .and_then(Value::as_u64)
        .unwrap_or(1440);
    let height = config
        .extra
        .get("browserViewportHeight")
        .and_then(Value::as_u64)
        .unwrap_or(900);
    args.push(format!("--window-size={width},{height}"));

    if let Some(url) = initial_url {
        let trimmed = url.trim();
        if !trimmed.is_empty() {
            args.push(trimmed.to_owned());
        }
    }

    Ok(args)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_browser_args_headless_mode() {
        let config = MintConfig::default();
        let args = build_browser_args(&config, "http://127.0.0.1:9222/json/list", None).unwrap();
        assert!(args.contains(&"--headless=new".to_string()));
        assert!(args.contains(&"--window-size=1440,900".to_string()));
        assert!(args.iter().any(|a| a.starts_with("--remote-debugging-port=9222")));
    }

    #[test]
    fn build_browser_args_headed_mode() {
        let mut config = MintConfig::default();
        config
            .extra
            .insert("browserAgentMode".into(), serde_json::json!("headed"));
        config
            .extra
            .insert("browserViewportWidth".into(), serde_json::json!(1920));
        config
            .extra
            .insert("browserViewportHeight".into(), serde_json::json!(1080));

        let args = build_browser_args(&config, "http://127.0.0.1:9222/json/list", Some("https://example.com")).unwrap();
        assert!(!args.contains(&"--headless=new".to_string()));
        assert!(args.contains(&"--window-size=1920,1080".to_string()));
        assert!(args.contains(&"https://example.com".to_string()));
    }
}

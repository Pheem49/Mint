//! Owned Chrome runtime shared by the CLI and the conversational agent.
use crate::MintConfig;
use serde_json::{Value, json};
use std::{
    io::Read,
    path::PathBuf,
    process::{Child, Command, Stdio},
    sync::{Arc, Mutex},
    time::Duration,
};

pub struct ManagedBrowser {
    child: Child,
    profile: PathBuf,
    pub endpoint: String,
    pub command: String,
    pub headless: bool,
    diagnostics: Arc<Mutex<String>>,
}

impl Drop for ManagedBrowser {
    fn drop(&mut self) {
        #[cfg(unix)]
        unsafe {
            libc::kill(-(self.child.id() as i32), libc::SIGKILL);
        }
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            let _ = Command::new("taskkill")
                .args(["/PID", &self.child.id().to_string(), "/T", "/F"])
                .creation_flags(0x08000000)
                .output();
        }
        let _ = self.child.kill();
        let _ = self.child.wait();
        let _ = std::fs::remove_dir_all(&self.profile);
    }
}

impl ManagedBrowser {
    pub fn metadata(&self) -> Value {
        json!({"mode":"headless","command":self.command,"diagnostics":self.diagnostics.lock().unwrap().as_str()})
    }

    pub async fn launch(config: &MintConfig) -> Result<Self, String> {
        let headless = true;
        let width = config
            .extra
            .get("browserViewportWidth")
            .and_then(Value::as_u64)
            .unwrap_or(1440)
            .clamp(320, 7680);
        let height = config
            .extra
            .get("browserViewportHeight")
            .and_then(Value::as_u64)
            .unwrap_or(900)
            .clamp(240, 4320);
        let profile =
            std::env::temp_dir().join(format!("mint-browser-run-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&profile).map_err(|e| e.to_string())?;
        let mut args = vec![
            "--remote-debugging-port=0".into(),
            "--remote-debugging-address=127.0.0.1".into(),
            format!("--user-data-dir={}", profile.display()),
            format!("--window-size={width},{height}"),
            "--no-first-run".into(),
            "--no-default-browser-check".into(),
            "--disable-background-networking".into(),
            "--disable-component-update".into(),
        ];
        if headless {
            args.push("--headless=new".into());
            args.push("--disable-gpu".into());
            if cfg!(target_os = "linux") {
                args.push("--no-sandbox".into());
                args.push("--disable-setuid-sandbox".into());
                args.push("--disable-dev-shm-usage".into());
            }
        }
        if let Some(extra) = config
            .extra
            .get("browserLaunchArgs")
            .and_then(Value::as_array)
        {
            for arg in extra.iter().filter_map(Value::as_str) {
                if arg.starts_with("--remote-debugging")
                    || arg.starts_with("--user-data-dir")
                    || arg.starts_with("--headless")
                {
                    let _ = std::fs::remove_dir_all(&profile);
                    return Err(
                        "browserLaunchArgs cannot override the managed endpoint, profile or mode"
                            .into(),
                    );
                }
                args.push(arg.into());
            }
        }
        args.push("about:blank".into());
        let configured = config
            .extra
            .get("browserExecutable")
            .and_then(Value::as_str)
            .filter(|s| !s.is_empty())
            .or_else(|| {
                config
                    .extra
                    .get("automationBrowser")
                    .and_then(Value::as_str)
                    .filter(|s| !s.is_empty() && *s != "chromium")
            });
        if configured.is_some_and(|exe| exe.to_ascii_lowercase().contains("firefox")) {
            let _ = std::fs::remove_dir_all(&profile);
            return Err("Browser automation requires Chrome/Chromium/Edge; set browserExecutable to a Chromium-based browser.".into());
        }
        let candidates: Vec<String> = if let Some(exe) = configured {
            vec![exe.into()]
        } else {
            let mut candidates: Vec<String> = if cfg!(target_os = "windows") {
                vec![
                    "chrome.exe".into(),
                    "msedge.exe".into(),
                    "chromium.exe".into(),
                ]
            } else if cfg!(target_os = "macos") {
                vec![
                    "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome".into(),
                    "/Applications/Microsoft Edge.app/Contents/MacOS/Microsoft Edge".into(),
                    "/Applications/Chromium.app/Contents/MacOS/Chromium".into(),
                    "google-chrome".into(),
                    "chromium".into(),
                ]
            } else {
                vec![
                    "google-chrome".into(),
                    "google-chrome-stable".into(),
                    "chromium".into(),
                    "chromium-browser".into(),
                    "microsoft-edge".into(),
                    "/opt/google/chrome/google-chrome".into(),
                    "/opt/google/chrome/chrome".into(),
                ]
            };
            #[cfg(target_os = "windows")]
            for base in ["PROGRAMFILES", "PROGRAMFILES(X86)", "LOCALAPPDATA"] {
                if let Some(dir) = std::env::var_os(base) {
                    for tail in [
                        "Google/Chrome/Application/chrome.exe",
                        "Microsoft/Edge/Application/msedge.exe",
                    ] {
                        candidates.push(
                            PathBuf::from(&dir)
                                .join(tail)
                                .to_string_lossy()
                                .into_owned(),
                        );
                    }
                }
            }
            candidates.shrink_to_fit();
            candidates
        };
        let diagnostics = Arc::new(Mutex::new(String::new()));
        let mut launched = None;
        for exe in candidates {
            let mut command = Command::new(&exe);
            command
                .args(&args)
                .stdout(Stdio::null())
                .stderr(Stdio::piped());
            #[cfg(unix)]
            {
                use std::os::unix::process::CommandExt;
                command.process_group(0);
            }
            #[cfg(windows)]
            {
                use std::os::windows::process::CommandExt;
                command.creation_flags(0x08000000);
            }
            if let Ok(mut child) = command.spawn() {
                let mut stderr = child.stderr.take().unwrap();
                let output = diagnostics.clone();
                std::thread::spawn(move || {
                    let mut bytes = [0; 4096];
                    while let Ok(n) = stderr.read(&mut bytes) {
                        if n == 0 {
                            break;
                        }
                        let mut text = output.lock().unwrap();
                        text.push_str(&String::from_utf8_lossy(&bytes[..n]));
                        if text.len() > 16_384 {
                            let mut cut = text.len() - 16_384;
                            while !text.is_char_boundary(cut) {
                                cut += 1;
                            }
                            text.drain(..cut);
                        }
                    }
                });
                let display = std::iter::once(exe.as_str())
                    .chain(args.iter().map(String::as_str))
                    .map(|s| {
                        shlex::try_quote(s)
                            .map(|s| s.into_owned())
                            .unwrap_or_else(|_| "<argument>".into())
                    })
                    .collect::<Vec<_>>()
                    .join(" ");
                launched = Some((child, display));
                break;
            }
        }
        let Some((child, command)) = launched else {
            let _ = std::fs::remove_dir_all(profile);
            return Err("Chrome/Chromium/Edge not found. Install a Chromium browser or set browserExecutable to its executable path.".into());
        };
        let mut runtime = Self {
            child,
            profile,
            endpoint: String::new(),
            command,
            headless,
            diagnostics,
        };
        for _ in 0..100 {
            if let Some(status) = runtime.child.try_wait().map_err(|e| e.to_string())? {
                return Err(format!(
                    "Chrome exited during startup ({status}): {}",
                    runtime.diagnostics.lock().unwrap()
                ));
            }
            if let Ok(port_file) =
                std::fs::read_to_string(runtime.profile.join("DevToolsActivePort"))
                && let Some(port) = port_file.lines().next().and_then(|p| p.parse::<u16>().ok())
            {
                let endpoint = format!("http://127.0.0.1:{port}/json/list");
                if super::cdp::fetch_pages_endpoint(&endpoint).await.is_ok() {
                    runtime.endpoint = endpoint;
                    return Ok(runtime);
                }
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
        Err(format!(
            "Chrome startup timed out: {}",
            runtime.diagnostics.lock().unwrap()
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    #[ignore = "requires local Chrome; launches an isolated real browser"]
    async fn managed_chrome_owns_its_profile_and_is_cleaned_on_drop() {
        let mut config = MintConfig::default();
        config
            .extra
            .insert("browserLaunchArgs".into(), json!(["--no-sandbox"]));
        let runtime = ManagedBrowser::launch(&config).await.unwrap();
        let profile = runtime.profile.clone();
        let endpoint = runtime.endpoint.clone();
        assert!(runtime.command.contains("--headless=new"));
        assert!(profile.exists());
        assert!(
            super::super::cdp::fetch_pages_endpoint(&endpoint)
                .await
                .is_ok()
        );
        drop(runtime);
        assert!(!profile.exists());
        assert!(
            super::super::cdp::fetch_pages_endpoint(&endpoint)
                .await
                .is_err()
        );
    }
}

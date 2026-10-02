//! Opt-in private HTTPS entry point for the existing Mint web UI.

use std::{
    net::{IpAddr, Ipv4Addr, SocketAddr},
    path::PathBuf,
    process::{Child, Command, Stdio},
    time::Duration,
};

use anyhow::{Context, Result, bail};
use serde_json::Value;

use crate::{BLUE, DIM, MINT, RESET, print_welcome_banner};

struct ChildGuard(Child);

impl Drop for ChildGuard {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

fn project_root() -> Result<PathBuf> {
    let cwd = std::env::current_dir()?;
    let exe = std::env::current_exe()?;
    exe.ancestors()
        .chain(cwd.ancestors())
        .find(|path| path.join("package.json").exists())
        .map(PathBuf::from)
        .context("Could not find the Mint project root (package.json)")
}

fn tailscale(args: &[&str]) -> Result<String> {
    let output = Command::new("tailscale").args(args).output().context(
        "Tailscale is not installed. Install it and sign in on this computer and your phone first",
    )?;
    if !output.status.success() {
        bail!(
            "tailscale {} failed: {}",
            args.join(" "),
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    Ok(String::from_utf8(output.stdout).context("Tailscale returned invalid UTF-8")?)
}

fn tailnet_url(status: &Value) -> Result<String> {
    if status["BackendState"] != "Running" {
        bail!("Tailscale is not connected. Sign in and connect this computer first");
    }
    let name = status["Self"]["DNSName"]
        .as_str()
        .map(|name| name.trim_end_matches('.'))
        .filter(|name| !name.is_empty())
        .context("Tailscale did not report a MagicDNS name; enable MagicDNS for this tailnet")?;
    Ok(format!("https://{name}"))
}

fn https_443_in_use(status: &Value) -> bool {
    let Some(config) = status.as_object() else {
        return true; // unknown status format: never risk replacing a route
    };
    if config
        .get("TCP")
        .unwrap_or(&Value::Null)
        .as_object()
        .is_some_and(|ports| ports.contains_key("443"))
        || config
            .get("Web")
            .unwrap_or(&Value::Null)
            .as_object()
            .is_some_and(|hosts| hosts.keys().any(|host| host.ends_with(":443")))
    {
        return true;
    }
    config
        .get("Foreground")
        .unwrap_or(&Value::Null)
        .as_object()
        .is_some_and(|sessions| sessions.values().any(https_443_in_use))
}

async fn wait_for_port(port: u16) -> Result<()> {
    let addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), port);
    for _ in 0..50 {
        if tokio::net::TcpStream::connect(addr).await.is_ok() {
            return Ok(());
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    bail!("Local port {port} did not become ready; check whether it is already in use")
}

async fn shutdown_signal() {
    #[cfg(unix)]
    {
        use tokio::signal::unix::{SignalKind, signal};
        if let Ok(mut terminate) = signal(SignalKind::terminate()) {
            tokio::select! {
                _ = tokio::signal::ctrl_c() => {},
                _ = terminate.recv() => {},
            }
            return;
        }
    }
    let _ = tokio::signal::ctrl_c().await;
}

pub async fn run() -> Result<()> {
    let root = project_root()?;
    if !root.join("out/web/index-web.html").exists() {
        bail!("Build the production Web UI first: npm run build:web");
    }
    let status: Value = serde_json::from_str(&tailscale(&["status", "--json"])?)
        .context("Could not read Tailscale status")?;
    let url = tailnet_url(&status)?;
    let serve_status: Value = serde_json::from_str(&tailscale(&["serve", "status", "--json"])?)
        .context("Could not read Tailscale Serve status")?;
    if https_443_in_use(&serve_status) {
        bail!("Tailscale Serve already uses HTTPS port 443; Mint will not replace it");
    }
    for port in [9000, 3000] {
        let addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), port);
        if tokio::net::TcpStream::connect(addr).await.is_ok() {
            bail!("Local port {port} is already in use; stop the existing service first");
        }
    }

    let config = mint_core::load_config()?;
    print_welcome_banner(&config);

    let vite = root.join("node_modules/vite/bin/vite.js");
    if !vite.exists() {
        bail!("Vite is missing. Run npm install in the Mint project first");
    }
    let web = ChildGuard(
        Command::new("node")
            .current_dir(&root)
            .env("MINT_TAILSCALE_HOST", url.trim_start_matches("https://"))
            .arg(vite)
            .args([
                "preview",
                "--config",
                "vite.config.web.ts",
                "--host",
                "127.0.0.1",
            ])
            .stdout(Stdio::null())
            .stderr(Stdio::inherit())
            .spawn()
            .context("Could not start the production Web UI")?,
    );
    wait_for_port(9000).await?;

    let api_addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 3000);
    let mut api = tokio::spawn(mint_core::api_server::start_api_server_on(api_addr));
    if let Err(error) = wait_for_port(3000).await {
        api.abort();
        return Err(error);
    }

    let mut serve = match Command::new("tailscale")
        .args(["serve", "--https=443", "http://127.0.0.1:9000"])
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .spawn()
    {
        Ok(child) => ChildGuard(child),
        Err(error) => {
            api.abort();
            return Err(error).context("Could not start Tailscale Serve");
        }
    };

    println!("\n{MINT}Mint Web is starting in your tailnet{RESET}\n");
    println!("    {BLUE}Mobile HTTPS:{RESET} {MINT}{url}{RESET}");
    println!("    {BLUE}Local Web UI:{RESET}  {MINT}http://127.0.0.1:9000{RESET}");
    println!("    {DIM}Open the HTTPS URL after Tailscale Serve reports it is available.{RESET}");
    println!("    {DIM}Press Ctrl+C to stop Mint Web and Tailscale Serve.{RESET}\n");

    let result = loop {
        tokio::select! {
            _ = shutdown_signal() => break Ok(()),
            outcome = &mut api => {
                break match outcome {
                    Ok(Ok(())) => Ok(()),
                    Ok(Err(error)) => Err(error.into()),
                    Err(error) => Err(error.into()),
                };
            }
            _ = tokio::time::sleep(Duration::from_secs(1)) => {
                if let Some(status) = serve.0.try_wait()? {
                    break Err(anyhow::anyhow!("Tailscale Serve stopped ({status}). Check that HTTPS certificates and Serve are enabled for this tailnet"));
                }
            }
        }
    };
    api.abort();
    drop(serve);
    drop(web);
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn requires_connected_tailnet_with_dns_name() {
        assert_eq!(
            tailnet_url(
                &json!({"BackendState":"Running","Self":{"DNSName":"mint.example.ts.net."}})
            )
            .unwrap(),
            "https://mint.example.ts.net"
        );
        assert!(tailnet_url(&json!({"BackendState":"Stopped"})).is_err());
        assert!(tailnet_url(&json!({"BackendState":"Running"})).is_err());
    }

    #[test]
    fn existing_serve_routes_are_never_overwritten() {
        assert!(!https_443_in_use(&json!({})));
        assert!(!https_443_in_use(&json!({"TCP":{"8443":{}}})));
        assert!(https_443_in_use(
            &json!({"Web":{"mint.example.ts.net:443":{}}})
        ));
        assert!(https_443_in_use(&json!({"TCP":{"443":{"HTTPS":true}}})));
        assert!(https_443_in_use(
            &json!({"Foreground":{"session":{"Web":{"mint.example.ts.net:443":{}}}}})
        ));
    }
}

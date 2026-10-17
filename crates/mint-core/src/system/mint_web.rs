//! Session-owned Mint Web background terminal.
use crate::MintConfig;
use std::{
    collections::HashMap,
    net::TcpListener,
    path::{Path, PathBuf},
    process::Command,
    sync::{LazyLock, Mutex},
};

static SERVERS: LazyLock<Mutex<HashMap<Option<String>, String>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

#[derive(Debug)]
struct WebLaunch {
    job_id: String,
    reused: bool,
}

fn launch(
    cwd: &Path,
    owner: Option<String>,
    config: &MintConfig,
    program: Command,
    ports: &[u16],
) -> Result<WebLaunch, String> {
    crate::assert_path_capability(cwd, crate::Capability::Write, config)
        .map_err(|e| e.to_string())?;
    let mut servers = SERVERS.lock().map_err(|e| e.to_string())?;
    if let Some(id) = servers.get(&owner)
        && let Ok(job) = crate::bg_shell::job_json(id, false)
    {
        if job["status"] == "running" {
            return Ok(WebLaunch {
                job_id: id.clone(),
                reused: true,
            });
        }
        if job["status"] == "stopping" {
            return Err("Mint Web is stopping; try again after it stops.".into());
        }
    }
    for port in ports {
        TcpListener::bind(("0.0.0.0", *port)).map_err(|_| {
            format!("Port {port} is already in use. Stop that server before starting /mint-web.")
        })?;
    }
    let context = crate::bg_shell::JobContext {
        workspace: cwd.to_path_buf(),
        owner: owner.clone(),
        ..Default::default()
    };
    let job = crate::bg_shell::with_context(context, || {
        crate::bg_shell::start_owned_background(cwd, config, "mint web", program)
    })
    .map_err(|e| e.to_string())?;
    servers.insert(owner, job.id.clone());
    Ok(WebLaunch {
        job_id: job.id,
        reused: false,
    })
}

fn executable() -> Result<PathBuf, String> {
    let name = if cfg!(windows) { "mint.exe" } else { "mint" };
    if let Ok(current) = std::env::current_exe() {
        if current.file_name().is_some_and(|n| n == name) {
            return Ok(current);
        }
        if let Some(parent) = current.parent() {
            let sibling = parent.join(name);
            if sibling.is_file() {
                return Ok(sibling);
            }
        }
    }
    if let Some(path) = std::env::var_os("PATH") {
        for dir in std::env::split_paths(&path) {
            let candidate = dir.join(name);
            if candidate.is_file() {
                return Ok(candidate);
            }
        }
    }
    Err("Mint CLI executable was not found. Install mint or place it beside the Desktop executable.".into())
}

pub fn command(
    cwd: &Path,
    rest: &str,
    owner: Option<String>,
    config: &MintConfig,
) -> Result<String, String> {
    if !rest.trim().is_empty() {
        return Err("Usage: /mint-web".into());
    }
    let mut program = Command::new(executable()?);
    program.args(["web", "--open"]);
    let launch = launch(cwd, owner, config, program, &[9000, 3000])?;
    let id = launch.job_id;
    let browser_notice = if launch.reused {
        "Mint Web is already running. Open the URL above to return to it."
    } else {
        "Your browser will open automatically when ready."
    };
    Ok(format!(
        "Mint Web background terminal: `{id}`\n\nMint Web: **http://localhost:9000**\n{browser_notice}\n\nIt stops when this Mint backend closes. Use `/shells show {id}` for startup logs or `/shells stop {id}` to stop it."
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(unix)]
    #[test]
    fn occupied_port_does_not_start_a_background_terminal() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let config = MintConfig {
            safety_enabled: false,
            ..Default::default()
        };
        let owner = Some(format!("occupied-port-{}", uuid::Uuid::new_v4()));
        let mut program = Command::new("bash");
        program.args(["-c", "sleep 30"]);
        let error = launch(Path::new("."), owner.clone(), &config, program, &[port]).unwrap_err();
        assert!(error.contains(&format!("Port {port} is already in use")));
        assert!(crate::bg_shell::list_json(&owner, None).is_empty());
    }

    #[cfg(unix)]
    #[test]
    fn launch_reuses_live_job_and_can_restart_after_stop() {
        let root = Path::new(".");
        let config = MintConfig {
            safety_enabled: false,
            ..Default::default()
        };
        let program = || {
            let mut p = Command::new("bash");
            p.args(["-c", "echo ready; cat"]);
            p
        };
        let first = launch(
            root,
            Some("web-launch-test".into()),
            &config,
            program(),
            &[],
        )
        .unwrap();
        let second = launch(
            root,
            Some("web-launch-test".into()),
            &config,
            program(),
            &[],
        )
        .unwrap();
        assert!(!first.reused);
        assert!(second.reused);
        assert_eq!(first.job_id, second.job_id);
        assert!(crate::bg_shell::accessible(
            &first.job_id,
            &Some("web-launch-test".into())
        ));
        crate::bg_shell::kill_job(&first.job_id).unwrap();
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(3);
        while crate::bg_shell::job_json(&first.job_id, false).unwrap()["status"] == "stopping" {
            assert!(std::time::Instant::now() < deadline);
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
        let next = launch(
            root,
            Some("web-launch-test".into()),
            &config,
            program(),
            &[],
        )
        .unwrap();
        assert!(!next.reused);
        assert_ne!(first.job_id, next.job_id);
        crate::bg_shell::kill_job(&next.job_id).unwrap();
    }
}

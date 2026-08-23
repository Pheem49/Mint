//! `mint gateway install` — registers `mint gateway start` as a systemd unit
//! so it survives reboots and keeps running without a login session attached
//! (the point of running Mint unattended on a VPS).

use std::{fs, path::PathBuf, process::Command};

use anyhow::{Context, Result, bail};

use crate::{ERROR, MINT, RESET};

const SERVICE_NAME: &str = "mint.service";

/// Generous but real ceiling on concurrent tasks/threads/subprocesses (the
/// browser automation and video/ffmpeg tooling can legitimately spawn a
/// handful) — this exists to contain a runaway fork-bomb-class bug, not to
/// constrain normal operation, so it's unconditional rather than a flag.
const TASKS_MAX: u32 = 512;

/// If the service crashes and `Restart=on-failure` brings it straight back
/// up into the *same* crash (a bad config value, a broken dependency, ...)
/// this stops systemd from restart-looping it indefinitely — after
/// `CRASH_LOOP_BURST` restarts inside `CRASH_LOOP_WINDOW_SECS`, systemd gives
/// up and leaves the unit failed instead of hammering the VPS (or a
/// rate-limited API) forever. `restarting_loop`'s own 5s backoff already
/// covers panics/errors *inside* a single bridge task (see `channels.rs`);
/// this is the outer safety net for the whole process crashing instead.
const CRASH_LOOP_BURST: u32 = 5;
const CRASH_LOOP_WINDOW_SECS: u32 = 120;

pub fn install(
    api_port: Option<u16>,
    system: bool,
    now: bool,
    memory_max: Option<String>,
) -> Result<()> {
    if !cfg!(target_os = "linux") {
        bail!("`mint gateway install` writes a systemd unit and is only supported on Linux.");
    }

    let exe = std::env::current_exe().context("could not determine the mint binary path")?;
    let mut exec_start = format!("{} gateway start", exe.display());
    if let Some(port) = api_port {
        exec_start.push_str(&format!(" --api-port {port}"));
    }
    let user = current_username()?;

    let (unit_path, scope): (PathBuf, &[&str]) = if system {
        (PathBuf::from("/etc/systemd/system").join(SERVICE_NAME), &[])
    } else {
        let config_dir = dirs::config_dir()
            .context("could not determine the user config directory (~/.config)")?;
        (
            config_dir.join("systemd/user").join(SERVICE_NAME),
            &["--user"],
        )
    };

    // `MemoryMax` is opt-in (via `--memory-max`) rather than a hardcoded
    // default: Mint's video/image tooling can legitimately need real memory
    // for a single task, and guessing a "safe" default risks the service
    // being OOM-killed mid-task on a small VPS for no benefit over just
    // leaving it unset.
    let memory_max_line = memory_max
        .as_deref()
        .map(|value| format!("MemoryMax={value}\n"))
        .unwrap_or_default();

    let unit_contents = if system {
        let home = dirs::home_dir()
            .map(|p| p.display().to_string())
            .unwrap_or_default();
        format!(
            "[Unit]\n\
             Description=Mint Gateway (headless chat bridges + cron)\n\
             After=network-online.target\n\
             Wants=network-online.target\n\
             StartLimitIntervalSec={CRASH_LOOP_WINDOW_SECS}\n\
             StartLimitBurst={CRASH_LOOP_BURST}\n\
             \n\
             [Service]\n\
             Type=simple\n\
             ExecStart={exec_start}\n\
             Restart=on-failure\n\
             RestartSec=5\n\
             User={user}\n\
             WorkingDirectory={home}\n\
             NoNewPrivileges=true\n\
             TasksMax={TASKS_MAX}\n\
             {memory_max_line}\
             \n\
             [Install]\n\
             WantedBy=multi-user.target\n"
        )
    } else {
        format!(
            "[Unit]\n\
             Description=Mint Gateway (headless chat bridges + cron)\n\
             After=network-online.target\n\
             Wants=network-online.target\n\
             StartLimitIntervalSec={CRASH_LOOP_WINDOW_SECS}\n\
             StartLimitBurst={CRASH_LOOP_BURST}\n\
             \n\
             [Service]\n\
             Type=simple\n\
             ExecStart={exec_start}\n\
             Restart=on-failure\n\
             RestartSec=5\n\
             NoNewPrivileges=true\n\
             TasksMax={TASKS_MAX}\n\
             {memory_max_line}\
             \n\
             [Install]\n\
             WantedBy=default.target\n"
        )
    };

    if let Some(parent) = unit_path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("could not create {}", parent.display()))?;
    }
    fs::write(&unit_path, unit_contents).with_context(|| {
        if system {
            format!(
                "could not write {} — re-run with sudo, or drop --system for a per-user unit",
                unit_path.display()
            )
        } else {
            format!("could not write {}", unit_path.display())
        }
    })?;
    println!("{MINT}✔ Wrote {}{RESET}", unit_path.display());

    run_systemctl(scope, &["daemon-reload"])?;
    run_systemctl(scope, &["enable", SERVICE_NAME])?;
    println!("{MINT}✔ Enabled {SERVICE_NAME} (starts on boot){RESET}");

    if now {
        run_systemctl(scope, &["start", SERVICE_NAME])?;
        println!("{MINT}✔ Started {SERVICE_NAME}{RESET}");
    }

    if system {
        println!("\nCheck status: systemctl status {SERVICE_NAME}");
        println!("Follow logs:  journalctl -u {SERVICE_NAME} -f");
        if !now {
            println!("Start it now: sudo systemctl start {SERVICE_NAME}");
        }
    } else {
        println!(
            "\nThis is a per-user service — by default it only runs while {user} is logged in. \
             To keep it running after logout/reboot with no login session, run:\n  \
             sudo loginctl enable-linger {user}"
        );
        println!("\nCheck status: systemctl --user status {SERVICE_NAME}");
        println!("Follow logs:  journalctl --user -u {SERVICE_NAME} -f");
        if !now {
            println!("Start it now: systemctl --user start {SERVICE_NAME}");
        }
    }

    Ok(())
}

fn run_systemctl(scope: &[&str], args: &[&str]) -> Result<()> {
    let status = Command::new("systemctl")
        .args(scope)
        .args(args)
        .status()
        .context("could not run `systemctl` — is systemd installed on this machine?")?;
    if !status.success() {
        let full = scope
            .iter()
            .chain(args)
            .copied()
            .collect::<Vec<_>>()
            .join(" ");
        bail!("{ERROR}`systemctl {full}` failed{RESET}");
    }
    Ok(())
}

fn current_username() -> Result<String> {
    for var in ["USER", "LOGNAME"] {
        if let Ok(user) = std::env::var(var)
            && !user.is_empty()
        {
            return Ok(user);
        }
    }
    let output = Command::new("id")
        .arg("-un")
        .output()
        .context("could not determine the current username")?;
    if !output.status.success() {
        bail!("could not determine the current username");
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

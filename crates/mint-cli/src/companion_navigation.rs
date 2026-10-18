//! Return to this terminal when its exact conversation/window is identifiable.
use mint_core::companion::navigation::{self, NavigationOutcome, Surface};
use std::{
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::Mutex,
};

static ACTIVE_CHAT: Mutex<String> = Mutex::new(String::new());

pub fn set_chat(chat_id: &str) {
    *ACTIVE_CHAT.lock().unwrap() = chat_id.to_owned();
}

pub fn configure(chat_id: &str) {
    set_chat(chat_id);
    let executable = std::env::current_exe().ok();
    let launch_dir = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let _embedded = std::env::var("TERM_PROGRAM").is_ok_and(|v| v == "vscode")
        || std::env::var_os("VSCODE_PID").is_some()
        || std::env::var_os("TMUX").is_some()
        || std::env::var_os("STY").is_some();
    #[cfg(target_os = "linux")]
    let window = if _embedded {
        None
    } else {
        std::env::var("WINDOWID")
            .ok()
            .filter(|id| id.parse::<u64>().is_ok_and(|id| id != 0))
    };
    #[cfg(target_os = "macos")]
    let tty = if !_embedded && std::env::var("TERM_PROGRAM").as_deref() == Ok("Apple_Terminal") {
        Command::new("tty")
            .stdin(Stdio::inherit())
            .output()
            .ok()
            .filter(|o| o.status.success())
            .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_owned())
    } else {
        None
    };
    navigation::configure(Surface::Tui, move |session, new_terminal| {
        if !new_terminal {
            if *ACTIVE_CHAT.lock().unwrap() != session.chat_id {
                return Ok(NavigationOutcome::NeedsTerminal);
            }
            #[cfg(target_os = "linux")]
            if let Some(window) = &window {
                let activated = Command::new("xdotool")
                    .args(["windowactivate", window])
                    .stdout(Stdio::null())
                    .stderr(Stdio::null())
                    .status()
                    .is_ok_and(|s| s.success());
                let focused = Command::new("xdotool")
                    .arg("getactivewindow")
                    .output()
                    .ok()
                    .is_some_and(|o| {
                        o.status.success()
                            && String::from_utf8_lossy(&o.stdout)
                                .trim()
                                .parse::<u64>()
                                .ok()
                                == window.parse::<u64>().ok()
                    });
                if activated && focused {
                    return Ok(NavigationOutcome::Opened);
                }
            }
            #[cfg(target_os = "macos")]
            if let Some(tty) = &tty {
                let tty = apple_string(tty);
                let script = format!(
                    "tell application \"Terminal\"\nrepeat with w in windows\nrepeat with t in tabs of w\nif tty of t is {tty} then\nset selected tab of w to t\nset index of w to 1\nactivate\nreturn \"focused\"\nend if\nend repeat\nend repeat\nend tell"
                );
                if Command::new("osascript")
                    .args(["-e", &script])
                    .output()
                    .ok()
                    .is_some_and(|o| {
                        o.status.success() && String::from_utf8_lossy(&o.stdout).trim() == "focused"
                    })
                {
                    return Ok(NavigationOutcome::Opened);
                }
            }
            return Ok(NavigationOutcome::NeedsTerminal);
        }
        let executable = executable
            .as_ref()
            .ok_or("Could not locate the Mint executable")?;
        let cwd = session
            .workspace
            .as_deref()
            .map(Path::new)
            .unwrap_or(&launch_dir);
        open_terminal(executable, cwd, &session.chat_id)?;
        Ok(NavigationOutcome::Opened)
    });
}

fn resume_args(executable: &Path, cwd: &Path, chat: &str) -> Vec<String> {
    vec![
        executable.to_string_lossy().into_owned(),
        "--cwd".into(),
        cwd.to_string_lossy().into_owned(),
        "--tui".into(),
        "--resume".into(),
        chat.into(),
    ]
}

fn open_terminal(executable: &Path, cwd: &Path, chat: &str) -> Result<(), String> {
    if !executable.is_file() {
        return Err("The Mint executable is no longer available. Restart Mint.".into());
    }
    if !cwd.is_dir() {
        return Err("The conversation workspace is no longer available.".into());
    }
    let args = resume_args(executable, cwd, chat);
    #[cfg(target_os = "linux")]
    {
        for (terminal, separator) in [
            ("x-terminal-emulator", "-e"),
            ("gnome-terminal", "--"),
            ("konsole", "-e"),
            ("xfce4-terminal", "--execute"),
            ("kitty", "--"),
            ("alacritty", "-e"),
            ("xterm", "-e"),
        ] {
            match Command::new(terminal)
                .arg(separator)
                .args(&args)
                .current_dir(cwd)
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn()
            {
                Ok(mut child) => {
                    std::thread::sleep(std::time::Duration::from_millis(150));
                    match child.try_wait().map_err(|error| error.to_string())? {
                        Some(status) if !status.success() => {
                            return Err(format!("{terminal} could not open the Mint conversation"));
                        }
                        Some(_) => return Ok(()),
                        None => {
                            std::thread::spawn(move || {
                                let _ = child.wait();
                            });
                            return Ok(());
                        }
                    }
                }
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
                Err(error) => return Err(format!("Could not open {terminal}: {error}")),
            }
        }
        return Err("No supported terminal was found. Open Mint with --tui --resume and this conversation ID.".into());
    }
    #[cfg(target_os = "macos")]
    {
        let command = args
            .iter()
            .map(|a| shell_quote(a))
            .collect::<Vec<_>>()
            .join(" ");
        let script = format!(
            "tell application \"Terminal\"\ndo script {}\nactivate\nend tell",
            apple_string(&command)
        );
        let status = Command::new("osascript")
            .args(["-e", &script])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .map_err(|e| e.to_string())?;
        return if status.success() {
            Ok(())
        } else {
            Err("Could not open Terminal".into())
        };
    }
    #[cfg(target_os = "windows")]
    {
        // Values travel through environment variables, never PowerShell source.
        let arguments = args[1..]
            .iter()
            .map(|a| windows_quote(a))
            .collect::<Vec<_>>()
            .join(" ");
        let status = Command::new("powershell.exe").args(["-NoProfile", "-NonInteractive", "-Command", "Start-Process -ErrorAction Stop -FilePath $env:MINT_COMPANION_EXE -ArgumentList $env:MINT_COMPANION_ARGS"])
            .env("MINT_COMPANION_EXE", executable).env("MINT_COMPANION_ARGS", arguments).stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null()).status().map_err(|e| e.to_string())?;
        return if status.success() {
            Ok(())
        } else {
            Err("Could not open a new Mint terminal".into())
        };
    }
    #[allow(unreachable_code)]
    Err("Opening terminals is unsupported on this platform".into())
}

#[cfg(target_os = "macos")]
fn shell_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\"'\"'"))
}
#[cfg(target_os = "macos")]
fn apple_string(value: &str) -> String {
    format!(
        "\"{}\"",
        value
            .replace('\\', "\\\\")
            .replace('"', "\\\"")
            .replace('\n', "\\n")
            .replace('\r', "\\r")
    )
}
#[cfg(target_os = "windows")]
fn windows_quote(value: &str) -> String {
    let mut result = String::from("\"");
    let mut slashes = 0;
    for character in value.chars() {
        if character == '\\' {
            slashes += 1;
            continue;
        }
        result.extend(std::iter::repeat_n(
            '\\',
            if character == '"' {
                slashes * 2 + 1
            } else {
                slashes
            },
        ));
        result.push(character);
        slashes = 0;
    }
    result.extend(std::iter::repeat_n('\\', slashes * 2));
    result.push('"');
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn resume_keeps_paths_and_chat_id_as_separate_arguments() {
        assert_eq!(
            resume_args(
                Path::new("/opt/Mint App/mint"),
                Path::new("/work/project one"),
                "cli::a ' & ไทย"
            ),
            [
                "/opt/Mint App/mint",
                "--cwd",
                "/work/project one",
                "--tui",
                "--resume",
                "cli::a ' & ไทย"
            ]
        );
    }
}

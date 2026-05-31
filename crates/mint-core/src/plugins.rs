use std::{
    fs,
    path::{Component, Path, PathBuf},
    process::Command,
    time::{SystemTime, UNIX_EPOCH},
};

use serde::Serialize;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum PluginError {
    #[error("unknown native plugin: {0}")]
    UnknownPlugin(String),
    #[error("invalid instruction for {plugin}: {message}")]
    InvalidInstruction {
        plugin: &'static str,
        message: String,
    },
    #[error("unable to run {program}: {source}")]
    Execute {
        program: &'static str,
        source: std::io::Error,
    },
    #[error("{program} failed: {message}")]
    Failed {
        program: &'static str,
        message: String,
    },
    #[error("unable to access note {path}: {source}")]
    Note {
        path: PathBuf,
        source: std::io::Error,
    },
}

#[derive(Debug, Clone, Serialize)]
pub struct NativePlugin {
    pub name: &'static str,
    pub description: &'static str,
}

pub fn native_plugins() -> Vec<NativePlugin> {
    vec![
        NativePlugin {
            name: "dev_tools",
            description: "Read git status, log, or branch information.",
        },
        NativePlugin {
            name: "docker",
            description: "List, start, stop, or restart local Docker containers.",
        },
        NativePlugin {
            name: "obsidian",
            description: "List, read, or append local Markdown notes.",
        },
        NativePlugin {
            name: "spotify",
            description: "Control Spotify through playerctl.",
        },
        NativePlugin {
            name: "system_metrics",
            description: "Read native RAM, CPU, and uptime metrics.",
        },
    ]
}

pub fn execute_native_plugin(name: &str, instruction: &str) -> Result<String, PluginError> {
    match name {
        "dev_tools" => dev_tools(instruction),
        "docker" => docker(instruction),
        "obsidian" => obsidian(instruction),
        "spotify" => spotify(instruction),
        "system_metrics" => system_metrics(instruction),
        _ => Err(PluginError::UnknownPlugin(name.into())),
    }
}

fn dev_tools(instruction: &str) -> Result<String, PluginError> {
    let lower = instruction.to_lowercase();
    let args = if lower.contains("status") {
        vec!["status", "--short"]
    } else if lower.contains("log") || lower.contains("commit") {
        vec!["log", "-n", "5", "--oneline"]
    } else if lower.contains("branch") {
        vec!["branch"]
    } else {
        return invalid("dev_tools", "expected status, log, or branch");
    };
    run("git", &args)
}

fn docker(instruction: &str) -> Result<String, PluginError> {
    let parts = instruction.split_whitespace().collect::<Vec<_>>();
    match parts.as_slice() {
        ["list"] => run("docker", &["ps", "--format", "{{.Names}} ({{.Status}})"]),
        [action, container]
            if matches!(*action, "start" | "stop" | "restart")
                && container.chars().all(|character| {
                    character.is_ascii_alphanumeric() || "_.-".contains(character)
                }) =>
        {
            run("docker", &[*action, *container])
        }
        _ => invalid(
            "docker",
            "expected list, start <container>, stop <container>, or restart <container>",
        ),
    }
}

fn obsidian(instruction: &str) -> Result<String, PluginError> {
    let directory = notes_directory()?;
    if instruction.trim() == "list" {
        let mut notes = fs::read_dir(&directory)
            .map_err(|source| PluginError::Note {
                path: directory.clone(),
                source,
            })?
            .flatten()
            .map(|entry| entry.file_name().to_string_lossy().to_string())
            .filter(|name| name.ends_with(".md"))
            .collect::<Vec<_>>();
        notes.sort();
        return Ok(notes.join("\n"));
    }
    if let Some(name) = instruction.strip_prefix("read:") {
        let path = note_path(&directory, name)?;
        return fs::read_to_string(&path).map_err(|source| PluginError::Note { path, source });
    }
    if let Some(payload) = instruction.strip_prefix("write:") {
        let Some((name, content)) = payload.split_once('|') else {
            return invalid("obsidian", "expected write: filename | content");
        };
        let path = note_path(&directory, name)?;
        let entry = format!("\n--- saved {} ---\n{}\n", timestamp(), content.trim());
        fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)
            .and_then(|mut file| std::io::Write::write_all(&mut file, entry.as_bytes()))
            .map_err(|source| PluginError::Note { path, source })?;
        return Ok("saved".into());
    }
    invalid(
        "obsidian",
        "expected list, read: filename, or write: filename | content",
    )
}

fn spotify(instruction: &str) -> Result<String, PluginError> {
    let parts = instruction.split_whitespace().collect::<Vec<_>>();
    let mut args = vec!["-p", "spotify"];
    match parts.as_slice() {
        [action @ ("play" | "pause" | "stop" | "next" | "previous")] => args.push(action),
        ["status"] | ["now_playing"] => args.extend([
            "metadata",
            "--format",
            "{{status}} | {{artist}} - {{title}}",
        ]),
        ["volume", level] => {
            let level = level
                .parse::<u8>()
                .ok()
                .filter(|level| *level <= 100)
                .ok_or_else(|| PluginError::InvalidInstruction {
                    plugin: "spotify",
                    message: "volume must be between 0 and 100".into(),
                })?;
            let level = format!("{:.2}", f32::from(level) / 100.0);
            return run("playerctl", &["-p", "spotify", "volume", &level]);
        }
        ["shuffle", state] if matches!(*state, "on" | "off" | "toggle") => {
            args.extend(["shuffle", state])
        }
        _ => {
            return invalid(
                "spotify",
                "expected play, pause, stop, next, previous, status, volume <0-100>, or shuffle <on|off|toggle>",
            );
        }
    }
    run("playerctl", &args)
}

fn system_metrics(instruction: &str) -> Result<String, PluginError> {
    let uptime = fs::read_to_string("/proc/uptime")
        .ok()
        .and_then(|raw| raw.split_whitespace().next()?.parse::<f64>().ok())
        .unwrap_or_default();
    let memory = fs::read_to_string("/proc/meminfo").unwrap_or_default();
    let total = meminfo_kib(&memory, "MemTotal").unwrap_or_default();
    let available = meminfo_kib(&memory, "MemAvailable").unwrap_or_default();
    let used = total.saturating_sub(available);
    let cpu_count = std::thread::available_parallelism()
        .map(|count| count.get())
        .unwrap_or(1);
    match instruction.trim() {
        "ram" => Ok(format!("RAM: {used} KiB used / {total} KiB total")),
        "cpu" => Ok(format!("CPU: {cpu_count} logical cores")),
        "uptime" => Ok(format!("uptime: {} minutes", (uptime / 60.0) as u64)),
        "" | "all" => Ok(format!(
            "RAM: {used} KiB used / {total} KiB total, CPU: {cpu_count} logical cores, uptime: {} minutes",
            (uptime / 60.0) as u64
        )),
        _ => invalid("system_metrics", "expected all, ram, cpu, or uptime"),
    }
}

fn notes_directory() -> Result<PathBuf, PluginError> {
    let path = dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("Documents")
        .join("Mint_Notes");
    fs::create_dir_all(&path).map_err(|source| PluginError::Note {
        path: path.clone(),
        source,
    })?;
    Ok(path)
}

fn note_path(directory: &Path, name: &str) -> Result<PathBuf, PluginError> {
    let mut name = name.trim().to_owned();
    if !name.ends_with(".md") {
        name.push_str(".md");
    }
    let path = PathBuf::from(&name);
    if path.components().count() != 1
        || !matches!(path.components().next(), Some(Component::Normal(_)))
    {
        return invalid("obsidian", "note name must not contain a path");
    }
    Ok(directory.join(path))
}

fn run(program: &'static str, args: &[&str]) -> Result<String, PluginError> {
    let output = Command::new(program)
        .args(args)
        .output()
        .map_err(|source| PluginError::Execute { program, source })?;
    if !output.status.success() {
        return Err(PluginError::Failed {
            program,
            message: String::from_utf8_lossy(&output.stderr).trim().to_owned(),
        });
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_owned())
}

fn invalid<T>(plugin: &'static str, message: impl Into<String>) -> Result<T, PluginError> {
    Err(PluginError::InvalidInstruction {
        plugin,
        message: message.into(),
    })
}

fn meminfo_kib(raw: &str, key: &str) -> Option<u64> {
    raw.lines()
        .find(|line| line.starts_with(key))?
        .split_whitespace()
        .nth(1)?
        .parse()
        .ok()
}

fn timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blocks_obsidian_path_traversal() {
        let error = note_path(Path::new("/tmp"), "../secret").unwrap_err();
        assert!(matches!(error, PluginError::InvalidInstruction { .. }));
    }

    #[test]
    fn rejects_unknown_plugins() {
        assert!(matches!(
            execute_native_plugin("missing", ""),
            Err(PluginError::UnknownPlugin(_))
        ));
    }
}

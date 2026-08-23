use std::path::{Path, PathBuf};

use super::*;

pub(super) fn validate_new_workspace_file(
    root: &Path,
    config: &MintConfig,
    path: &Path,
) -> Result<(), OrchestrationError> {
    let root = assert_path_capability(root, Capability::Write, config)
        .map_err(|e| OrchestrationError::Agent(e.to_string()))?;
    let target = assert_path_capability(&root.join(path), Capability::Write, config)
        .map_err(|e| OrchestrationError::Agent(e.to_string()))?;
    if !target.starts_with(&root) {
        return Err(OrchestrationError::Agent(format!(
            "write_file path escapes workspace root: {}",
            target.display()
        )));
    }
    if target.exists() {
        return Err(OrchestrationError::Agent(format!(
            "write_file can only create new files. Use apply_patch for existing file: {}",
            target.display()
        )));
    }
    Ok(())
}

pub(super) fn run_git(root: &Path, args: &[&str]) -> Result<String, OrchestrationError> {
    let output = Command::new("git")
        .args(args)
        .current_dir(root)
        .output()
        .map_err(|e| OrchestrationError::Agent(format!("unable to run git: {e}")))?;
    Ok(format!(
        "exit: {}\nstdout:\n{}\nstderr:\n{}",
        output.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    ))
}

pub(super) fn detect_project(root: &Path) -> Value {
    let mut languages = Vec::new();
    let mut managers = Vec::new();
    let mut diagnostics = Vec::new();
    if root.join("Cargo.toml").exists() {
        languages.push("rust");
        managers.push("cargo");
        diagnostics.push("cargo check");
    }
    if root.join("package.json").exists() {
        languages.push("javascript/typescript");
        managers.push(if root.join("pnpm-lock.yaml").exists() {
            "pnpm"
        } else if root.join("yarn.lock").exists() {
            "yarn"
        } else {
            "npm"
        });
        diagnostics.push("npm run build or npm run typecheck");
    }
    if root.join("pyproject.toml").exists() || root.join("requirements.txt").exists() {
        languages.push("python");
        managers.push("pip/uv");
        diagnostics.push("pytest or python -m compileall");
    }
    serde_json::json!({
        "root": root,
        "languages": languages,
        "packageManagers": managers,
        "diagnostics": diagnostics,
    })
}

pub(super) fn list_tests(root: &Path, config: &MintConfig) -> Result<Value, OrchestrationError> {
    let files = list_code_files(root, usize::MAX, config)
        .map_err(|e| OrchestrationError::Agent(e.to_string()))?;
    let test_files = files
        .into_iter()
        .filter(|file| {
            let path = file.path.to_string_lossy();
            path.contains("/tests/")
                || path.ends_with("_test.rs")
                || path.ends_with(".test.ts")
                || path.ends_with(".test.tsx")
                || path.ends_with(".spec.ts")
                || path.ends_with(".spec.tsx")
                || path.ends_with("_test.py")
        })
        .map(|file| file.path)
        .collect::<Vec<_>>();
    let package_scripts = package_test_scripts(root);
    Ok(serde_json::json!({
        "testFiles": test_files,
        "packageScripts": package_scripts,
        "cargo": root.join("Cargo.toml").exists(),
    }))
}

pub(super) fn package_test_scripts(root: &Path) -> BTreeMap<String, String> {
    let path = root.join("package.json");
    let Ok(raw) = std::fs::read_to_string(path) else {
        return BTreeMap::new();
    };
    let Ok(value) = serde_json::from_str::<Value>(&raw) else {
        return BTreeMap::new();
    };
    value
        .get("scripts")
        .and_then(Value::as_object)
        .into_iter()
        .flatten()
        .filter(|(name, _)| {
            let lower = name.to_ascii_lowercase();
            lower.contains("test")
                || lower.contains("check")
                || lower.contains("lint")
                || lower.contains("build")
                || lower.contains("type")
        })
        .filter_map(|(name, command)| Some((name.clone(), command.as_str()?.to_owned())))
        .collect()
}

pub(super) async fn read_diagnostics(
    root: &Path,
    config: &MintConfig,
    chat_id: &str,
) -> Result<String, OrchestrationError> {
    let command = if root.join("Cargo.toml").exists() {
        Some("cargo check")
    } else {
        let scripts = package_test_scripts(root);
        if scripts.contains_key("typecheck") {
            Some("npm run -s typecheck")
        } else if scripts.contains_key("check") {
            Some("npm run -s check")
        } else if scripts.contains_key("build") {
            Some("npm run -s build")
        } else {
            None
        }
    };
    match command {
        Some(command) => run_shell(root, config, chat_id, command).await,
        None => Ok("No diagnostics command detected.".into()),
    }
}

pub(super) fn view_image(path: &Path, config: &MintConfig) -> Result<String, OrchestrationError> {
    let path = assert_path_capability(path, Capability::Read, config)
        .map_err(|e| OrchestrationError::Agent(e.to_string()))?;
    let extension = path
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    let mime = match extension.as_str() {
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "svg" => "image/svg+xml",
        _ => {
            return Err(OrchestrationError::Agent(format!(
                "unsupported image type: {}",
                path.display()
            )));
        }
    };
    let metadata = std::fs::metadata(&path)
        .map_err(|e| OrchestrationError::Agent(format!("cannot stat image: {e}")))?;
    if metadata.len() > 2_000_000 {
        return Ok(format!(
            "Image exists but is too large to inline ({} bytes): {}",
            metadata.len(),
            path.display()
        ));
    }
    let bytes = std::fs::read(&path)
        .map_err(|e| OrchestrationError::Agent(format!("cannot read image: {e}")))?;
    Ok(format!(
        "data:{mime};base64,{}",
        BASE64_STANDARD.encode(bytes)
    ))
}

// `async` and dispatched onto tokio's blocking-thread-pool via
// `spawn_blocking` (not called synchronously in place) because
// `run_shell_command` is a plain blocking `std::process::Command` call —
// possibly for as long as the command itself runs (`du -xhd1 /`, a build,
// a sleep, ...). `execute_tool`'s caller races this whole future against
// `wait_for_escape_interrupt` inside one `tokio::select!` in `mint-cli`,
// which only *interleaves* futures on the current task rather than giving
// each its own OS thread — a blocking call left in place here would never
// yield control back to the executor, so the Esc-watcher (and, in the CLI,
// keystrokes typed into the mid-turn input box) would starve for the
// command's entire duration, not just get delayed. `spawn_blocking` moves
// the actual blocking work to a dedicated thread so this task's `.await`
// point here is a real yield.
pub(super) async fn run_shell(
    root: &Path,
    config: &MintConfig,
    chat_id: &str,
    command: &str,
) -> Result<String, OrchestrationError> {
    let root = root.to_path_buf();
    let config = config.clone();
    let chat_id = chat_id.to_owned();
    let command = command.to_owned();
    let output = {
        let command = command.clone();
        tokio::task::spawn_blocking(move || {
            run_shell_command(&command, &root, true, &config, Some(&chat_id))
        })
        .await
        .map_err(|e| OrchestrationError::Agent(format!("shell command task panicked: {e}")))?
        .map_err(|e| OrchestrationError::Agent(e.to_string()))?
    };
    let status_str = output
        .status
        .map_or_else(|| "unknown".into(), |status| status.to_string());

    let mut hint = "";
    let cmd_lower = command.to_lowercase();
    if output.success
        && (cmd_lower.contains("open")
            || cmd_lower.contains("launch")
            || cmd_lower.contains("chrome")
            || cmd_lower.contains("firefox"))
    {
        hint = "\nNote: Opening URLs, files, folders, or launching applications are background processes. Even if there are warnings or stdout/stderr outputs, since the command exited successfully with status 0, the operation has succeeded and you should now use the 'finish' action to inform the user.";
    }

    let warning_line = output
        .sandbox_warning
        .as_deref()
        .map(|warning| format!("\n[Warning] {warning}"))
        .unwrap_or_default();

    Ok(format!(
        "exit: {}\nmode: {}\nsandboxed: {}{}\nstdout:\n{}\nstderr:\n{}{}",
        status_str, output.mode, output.sandboxed, warning_line, output.stdout, output.stderr, hint
    ))
}

pub(super) fn workspace_context(root: &Path) -> String {
    let mut context = String::from("Automatic workspace context:\n");
    context.push_str(&format!(
        "Git status:\n{}\n",
        command_output(root, "git", &["status", "--short"])
    ));
    context.push_str(&format!(
        "Diff summary:\n{}\n",
        command_output(root, "git", &["diff", "--stat"])
    ));
    context.push_str(&format!("Package scripts:\n{}\n", package_scripts(root)));
    context
}

pub(super) fn command_output(root: &Path, program: &str, args: &[&str]) -> String {
    use std::process::Command;
    match Command::new(program).args(args).current_dir(root).output() {
        Ok(output) if output.status.success() => {
            let value = String::from_utf8_lossy(&output.stdout);
            if value.trim().is_empty() {
                "(none)".into()
            } else {
                truncate(&value).trim().into()
            }
        }
        _ => "(unavailable)".into(),
    }
}

pub(super) fn package_scripts(root: &Path) -> String {
    let Ok(raw) = std::fs::read_to_string(root.join("package.json")) else {
        return "(none)".into();
    };
    let Ok(value) = serde_json::from_str::<Value>(&raw) else {
        return "(invalid package.json)".into();
    };
    let Some(scripts) = value.get("scripts").and_then(Value::as_object) else {
        return "(none)".into();
    };
    scripts
        .iter()
        .map(|(name, command)| format!("{name}: {}", command.as_str().unwrap_or_default()))
        .collect::<Vec<_>>()
        .join("\n")
}

pub(super) fn list_directory_entries(
    path: &Path,
    limit: usize,
    config: &MintConfig,
) -> Result<Vec<AgentDirectoryEntry>, OrchestrationError> {
    let path = assert_path_capability(path, Capability::Read, config)
        .map_err(|e| OrchestrationError::Agent(e.to_string()))?;
    if !path.is_dir() {
        return Err(OrchestrationError::Agent(format!(
            "path is not a directory: {}",
            path.display()
        )));
    }

    let mut entries = Vec::new();
    let read_dir = std::fs::read_dir(&path).map_err(|e| {
        OrchestrationError::Agent(format!(
            "unable to read directory {}: {}",
            path.display(),
            e
        ))
    })?;
    for entry in read_dir.take(limit.max(1)) {
        let entry = entry.map_err(|e| {
            OrchestrationError::Agent(format!("unable to read directory entry: {e}"))
        })?;
        let entry_path = entry.path();
        let file_type = entry.file_type().map_err(|e| {
            OrchestrationError::Agent(format!(
                "unable to read file type for {}: {}",
                entry_path.display(),
                e
            ))
        })?;
        let size = if file_type.is_file() {
            entry.metadata().ok().map(|metadata| metadata.len())
        } else {
            None
        };
        entries.push(AgentDirectoryEntry {
            name: entry.file_name().to_string_lossy().into_owned(),
            path: entry_path,
            kind: if file_type.is_dir() {
                "directory"
            } else if file_type.is_file() {
                "file"
            } else if file_type.is_symlink() {
                "symlink"
            } else {
                "other"
            },
            size,
        });
    }
    entries.sort_by(|a, b| {
        a.kind
            .cmp(b.kind)
            .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
    });
    Ok(entries)
}

pub(super) fn agent_read_path(
    root: &Path,
    value: &str,
    config: &MintConfig,
) -> Result<PathBuf, OrchestrationError> {
    let trimmed = value.trim();
    if trimmed.is_empty() || trimmed == "." {
        return workspace_path(root, ".");
    }
    if let Ok(path) = workspace_path(root, trimmed) {
        return Ok(path);
    }

    let requested = Path::new(trimmed);
    let mut candidates = Vec::new();
    if let Some(home) = dirs::home_dir() {
        if trimmed == "~" {
            candidates.push(home.clone());
        } else if let Some(rest) = trimmed.strip_prefix("~/") {
            candidates.push(home.join(rest));
        } else if requested.components().count() == 1 {
            candidates.push(home.join(trimmed));
        }
    }
    if requested.is_absolute() {
        candidates.push(requested.to_path_buf());
    }

    for candidate in candidates {
        let Ok(path) = candidate.canonicalize() else {
            continue;
        };
        if assert_path_capability(&path, Capability::Read, config).is_ok() {
            return Ok(path);
        }
    }

    Err(OrchestrationError::Agent(format!(
        "unable to resolve readable path: {trimmed}"
    )))
}

pub(super) fn workspace_path(root: &Path, value: &str) -> Result<PathBuf, OrchestrationError> {
    let path = root.join(if value.trim().is_empty() { "." } else { value });
    let path = path.canonicalize().map_err(|e| {
        OrchestrationError::Agent(format!(
            "unable to resolve workspace path {}: {}",
            path.display(),
            e
        ))
    })?;
    if !path.starts_with(root) {
        return Err(OrchestrationError::Agent(format!(
            "path is outside workspace: {}",
            path.display()
        )));
    }
    Ok(path)
}

pub(super) fn required<'a>(value: &'a str, name: &str) -> Result<&'a str, OrchestrationError> {
    if value.trim().is_empty() {
        return Err(OrchestrationError::Agent(format!("{} is required", name)));
    }
    Ok(value)
}

pub(super) fn truncate(value: &str) -> String {
    if value.len() <= MAX_OBSERVATION_BYTES {
        value.into()
    } else {
        let mut end = MAX_OBSERVATION_BYTES;
        while !value.is_char_boundary(end) {
            end -= 1;
        }
        format!("{}\n...<truncated>", &value[..end])
    }
}

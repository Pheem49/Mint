use std::{
    path::{Path, PathBuf},
    process::{Command, Output},
};

use serde::Serialize;
use thiserror::Error;

use crate::{
    Capability, MintConfig, SafetyError, SafetyTier, ShellCommandMode, assert_path_capability,
    classify_shell_command, shell_mode_allowed,
};

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ShellOutput {
    pub command: String,
    pub cwd: PathBuf,
    pub mode: String,
    pub status: Option<i32>,
    pub success: bool,
    pub sandboxed: bool,
    /// Set when a mutating/network command ran unconfined because sandboxing was
    /// unavailable (rather than intentionally disabled via `sandboxMode: "off"`),
    /// so callers can surface this instead of it happening silently.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sandbox_warning: Option<String>,
    pub stdout: String,
    pub stderr: String,
}

/// Whether OS-level sandboxing is actually usable for the configured
/// `sandbox_command`, independent of whether `sandbox_mode` calls for it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum SandboxAvailability {
    Available,
    BinaryMissing,
    UnsupportedPlatform,
}

pub fn sandbox_availability(config: &MintConfig) -> SandboxAvailability {
    if cfg!(not(any(target_os = "linux", target_os = "macos"))) {
        return SandboxAvailability::UnsupportedPlatform;
    }
    let sandbox = config.sandbox_command.trim();
    if sandbox.is_empty() || !command_exists(sandbox) {
        return SandboxAvailability::BinaryMissing;
    }
    SandboxAvailability::Available
}

#[derive(Debug, Error)]
pub enum ShellError {
    #[error("shell command requires explicit approval: {0}")]
    ApprovalRequired(String),
    #[error("blocked unsafe shell command ({reason}): {command}")]
    Blocked { command: String, reason: String },
    #[error("shell command mode '{mode}' is not allowed by policy: {command}")]
    ModeDenied { command: String, mode: String },
    #[error("shell working directory must be a directory: {0}")]
    InvalidWorkingDirectory(PathBuf),
    #[error(transparent)]
    Safety(#[from] SafetyError),
    #[error("sandbox mode is enforced but sandbox command '{0}' is unavailable")]
    SandboxUnavailable(String),
    #[error("unable to execute shell command: {0}")]
    Execute(#[from] std::io::Error),
}

pub fn run_shell_command(
    command: &str,
    cwd: &Path,
    approved: bool,
    config: &MintConfig,
) -> Result<ShellOutput, ShellError> {
    let classification = classify_shell_command(command);
    if classification.tier == SafetyTier::Blocked {
        return Err(ShellError::Blocked {
            command: command.into(),
            reason: classification.reason,
        });
    }
    if config.safety_enabled && !shell_mode_allowed(config, classification.mode) {
        return Err(ShellError::ModeDenied {
            command: command.into(),
            mode: classification.mode.as_str().into(),
        });
    }
    if !approved {
        return Err(ShellError::ApprovalRequired(command.into()));
    }

    let cwd = assert_path_capability(cwd, Capability::Write, config)?;
    if !cwd.is_dir() {
        return Err(ShellError::InvalidWorkingDirectory(cwd));
    }

    let prepared_cmd = prepare_shell_command(command);
    let sandbox_mode = config.sandbox_mode.trim().to_ascii_lowercase();
    let mut sandbox_warning = None;
    if config.safety_enabled && sandbox_mode != "off" {
        if let Some(output) = run_in_sandbox(&prepared_cmd, &cwd, config)? {
            return Ok(shell_output(
                command,
                cwd,
                classification.mode.as_str(),
                true,
                output,
                None,
            ));
        }
        if sandbox_mode == "enforce" {
            return Err(ShellError::SandboxUnavailable(
                config.sandbox_command.clone(),
            ));
        }
        // Falling back to unconfined execution (`sandboxMode` is the default
        // "prefer" or some other non-"enforce" value). Only warn for
        // higher-risk commands — read-only commands carry little risk either
        // way, and warning on every `ls`/`cat` would be noise.
        if matches!(
            classification.mode,
            ShellCommandMode::Mutating | ShellCommandMode::Network
        ) {
            sandbox_warning = Some(match sandbox_availability(config) {
                SandboxAvailability::UnsupportedPlatform => {
                    "Sandboxing is not supported on this platform; this command ran unconfined."
                        .to_string()
                }
                SandboxAvailability::BinaryMissing => format!(
                    "Sandbox command '{}' was not found; this command ran unconfined. \
                     Install it, or set sandboxMode to \"enforce\" to block instead of \
                     falling back.",
                    config.sandbox_command.trim()
                ),
                SandboxAvailability::Available => {
                    "Sandbox was available but did not run; this command ran unconfined."
                        .to_string()
                }
            });
        }
    }

    let output = shell_command(&prepared_cmd).current_dir(&cwd).output()?;
    Ok(shell_output(
        command,
        cwd,
        classification.mode.as_str(),
        false,
        output,
        sandbox_warning,
    ))
}

fn prepare_shell_command(command: &str) -> String {
    let trimmed = command.trim();
    if trimmed.ends_with('&') && !trimmed.ends_with("&&") {
        #[cfg(not(target_os = "windows"))]
        {
            let inner = trimmed.trim_end_matches('&').trim();
            format!("nohup {inner} >/dev/null 2>&1 &")
        }
        #[cfg(target_os = "windows")]
        {
            command.to_string()
        }
    } else {
        command.to_string()
    }
}

fn run_in_sandbox(
    command: &str,
    cwd: &Path,
    config: &MintConfig,
) -> Result<Option<Output>, ShellError> {
    #[cfg(target_os = "linux")]
    {
        let sandbox = config.sandbox_command.trim();
        if sandbox.is_empty() || !command_exists(sandbox) {
            return Ok(None);
        }
        let mut process = Command::new(sandbox);
        process
            .args(["--die-with-parent", "--ro-bind", "/", "/", "--dev", "/dev"])
            .args(["--proc", "/proc", "--tmpfs", "/tmp"]);
        for root in writable_roots(config, cwd) {
            process.arg("--bind").arg(&root).arg(&root);
        }
        let output = process
            .arg("--chdir")
            .arg(cwd)
            .args(["bash", "-lc", command])
            .output()?;
        Ok(Some(output))
    }

    #[cfg(target_os = "macos")]
    {
        let sandbox = config.sandbox_command.trim();
        if sandbox.is_empty() || !command_exists(sandbox) {
            return Ok(None);
        }
        let output = Command::new(sandbox)
            .arg("-p")
            .arg(mac_sandbox_profile(config, cwd))
            .args(["bash", "-lc", command])
            .current_dir(cwd)
            .output()?;
        return Ok(Some(output));
    }

    #[cfg(not(any(target_os = "linux", target_os = "macos")))]
    {
        let _ = (command, cwd, config);
        Ok(None)
    }
}

#[cfg(target_os = "macos")]
fn mac_sandbox_profile(config: &MintConfig, cwd: &Path) -> String {
    let mut read_roots = vec![
        cwd.to_path_buf(),
        PathBuf::from("/bin"),
        PathBuf::from("/sbin"),
        PathBuf::from("/usr"),
        PathBuf::from("/System"),
        PathBuf::from("/Library"),
    ];
    read_roots.extend(config.allowed_read_paths.iter().cloned());
    read_roots.extend(config.allowed_write_paths.iter().cloned());

    let mut write_roots = vec![cwd.to_path_buf(), std::env::temp_dir()];
    write_roots.extend(config.allowed_write_paths.iter().cloned());

    format!(
        "(version 1)\n\
         (deny default)\n\
         (allow process*)\n\
         (allow sysctl-read)\n\
         (allow signal (target self))\n\
         (allow file-read-metadata)\n\
         (allow file-read*\n{})\n\
         (allow file-write*\n{})",
        sandbox_subpaths(read_roots),
        sandbox_subpaths(write_roots),
    )
}

#[cfg(target_os = "macos")]
fn sandbox_subpaths(mut roots: Vec<PathBuf>) -> String {
    roots.sort();
    roots.dedup();
    roots
        .into_iter()
        .filter(|root| root.exists())
        .map(|root| {
            format!(
                "  (subpath \"{}\")",
                root.to_string_lossy()
                    .replace('\\', "\\\\")
                    .replace('"', "\\\"")
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[cfg(target_os = "linux")]
fn writable_roots(config: &MintConfig, cwd: &Path) -> Vec<PathBuf> {
    let mut roots = config
        .allowed_write_paths
        .iter()
        .filter(|root| root.exists())
        .cloned()
        .collect::<Vec<_>>();
    if !roots.iter().any(|root| cwd.starts_with(root)) {
        roots.push(cwd.to_path_buf());
    }
    roots.sort();
    roots.dedup();
    roots
}

fn command_exists(command: &str) -> bool {
    let Some(path_os) = std::env::var_os("PATH") else {
        return false;
    };
    let extensions: &[&str] = if cfg!(target_os = "windows") {
        &[".exe", ".cmd", ".bat", ".com", ""]
    } else {
        &[""]
    };
    for dir in std::env::split_paths(&path_os) {
        for ext in extensions {
            let candidate = dir.join(format!("{command}{ext}"));
            if candidate.is_file() {
                #[cfg(unix)]
                {
                    use std::os::unix::fs::PermissionsExt;
                    if let Ok(metadata) = candidate.metadata() {
                        if metadata.permissions().mode() & 0o111 != 0 {
                            return true;
                        }
                    }
                }
                #[cfg(not(unix))]
                {
                    return true;
                }
            }
        }
    }
    false
}

pub(crate) fn shell_command(command: &str) -> Command {
    #[cfg(target_os = "windows")]
    {
        let mut process = Command::new("powershell.exe");
        process.args([
            "-NoLogo",
            "-NoProfile",
            "-NonInteractive",
            "-ExecutionPolicy",
            "Bypass",
            "-Command",
            command,
        ]);
        process
    }
    #[cfg(not(target_os = "windows"))]
    {
        let mut process = Command::new("bash");
        process.args(["-lc", command]);
        process
    }
}

fn shell_output(
    command: &str,
    cwd: PathBuf,
    mode: &str,
    sandboxed: bool,
    output: Output,
    sandbox_warning: Option<String>,
) -> ShellOutput {
    ShellOutput {
        command: command.into(),
        cwd,
        mode: mode.into(),
        status: output.status.code(),
        success: output.status.success(),
        sandboxed,
        sandbox_warning,
        stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn local_config() -> MintConfig {
        MintConfig {
            safety_enabled: false,
            sandbox_mode: "off".into(),
            ..MintConfig::default()
        }
    }

    #[test]
    fn requires_explicit_approval() {
        let error =
            run_shell_command("printf mint", Path::new("."), false, &local_config()).unwrap_err();
        assert!(matches!(error, ShellError::ApprovalRequired(_)));
    }

    #[test]
    fn blocks_destructive_commands_even_when_approved() {
        let error = run_shell_command(
            "git reset --hard HEAD",
            Path::new("."),
            true,
            &local_config(),
        )
        .unwrap_err();
        assert!(matches!(error, ShellError::Blocked { .. }));
    }

    #[test]
    fn runs_an_approved_local_command() {
        let output =
            run_shell_command("printf mint", Path::new("."), true, &local_config()).unwrap();
        assert!(output.success);
        assert!(!output.sandboxed);
        assert_eq!(output.stdout, "mint");
    }

    #[test]
    fn reports_non_zero_exit_status() {
        let output = run_shell_command(
            "printf failure >&2; exit 7",
            Path::new("."),
            true,
            &local_config(),
        )
        .unwrap();
        assert!(!output.success);
        assert_eq!(output.status, Some(7));
        assert_eq!(output.stderr, "failure");
    }

    #[test]
    fn test_command_exists() {
        assert!(command_exists("sh") || command_exists("bash") || command_exists("cmd"));
        assert!(!command_exists("non_existent_binary_xyz_12345"));
    }

    #[test]
    fn sandbox_availability_reports_binary_missing_for_bogus_command() {
        let config = MintConfig {
            sandbox_command: "non_existent_binary_xyz_12345".into(),
            ..MintConfig::default()
        };
        #[cfg(any(target_os = "linux", target_os = "macos"))]
        assert_eq!(
            sandbox_availability(&config),
            SandboxAvailability::BinaryMissing
        );
        #[cfg(not(any(target_os = "linux", target_os = "macos")))]
        assert_eq!(
            sandbox_availability(&config),
            SandboxAvailability::UnsupportedPlatform
        );
    }

    #[test]
    fn mutating_command_without_sandbox_binary_carries_a_warning() {
        let mut config = MintConfig {
            safety_enabled: true,
            sandbox_mode: "prefer".into(),
            sandbox_command: "non_existent_binary_xyz_12345".into(),
            ..MintConfig::default()
        };
        config
            .extra
            .insert("allowedShellModes".into(), serde_json::json!(["*"]));
        let out_path = std::env::temp_dir().join("mint-shell-warning-test.txt");
        let output = run_shell_command(
            &format!("printf mint > {}", out_path.display()),
            Path::new("."),
            true,
            &config,
        )
        .unwrap();
        assert!(!output.sandboxed);
        assert!(output.sandbox_warning.is_some());
        let _ = std::fs::remove_file(&out_path);
    }

    #[test]
    fn read_only_command_without_sandbox_binary_carries_no_warning() {
        let mut config = MintConfig {
            safety_enabled: true,
            sandbox_mode: "prefer".into(),
            sandbox_command: "non_existent_binary_xyz_12345".into(),
            ..MintConfig::default()
        };
        config
            .extra
            .insert("allowedShellModes".into(), serde_json::json!(["*"]));
        let output = run_shell_command("pwd", Path::new("."), true, &config).unwrap();
        assert!(output.sandbox_warning.is_none());
    }

    #[test]
    fn test_background_command_does_not_hang() {
        let start = std::time::Instant::now();
        let output = run_shell_command("sleep 5 &", Path::new("."), true, &local_config()).unwrap();
        assert!(output.success);
        assert!(start.elapsed() < std::time::Duration::from_secs(2));
    }
}

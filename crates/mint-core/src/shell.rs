use std::{
    path::{Path, PathBuf},
    process::{Command, Output},
};

use serde::Serialize;
use thiserror::Error;

use crate::{
    Capability, MintConfig, SafetyError, SafetyTier, assert_path_capability,
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
    pub stdout: String,
    pub stderr: String,
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

    let sandbox_mode = config.sandbox_mode.trim().to_ascii_lowercase();
    if config.safety_enabled && sandbox_mode != "off" {
        if let Some(output) = run_in_sandbox(command, &cwd, config)? {
            return Ok(shell_output(
                command,
                cwd,
                classification.mode.as_str(),
                true,
                output,
            ));
        }
        if sandbox_mode == "enforce" {
            return Err(ShellError::SandboxUnavailable(
                config.sandbox_command.clone(),
            ));
        }
    }

    let output = shell_command(command).current_dir(&cwd).output()?;
    Ok(shell_output(
        command,
        cwd,
        classification.mode.as_str(),
        false,
        output,
    ))
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
        return Ok(Some(output));
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

#[cfg(any(target_os = "linux", target_os = "macos"))]
fn command_exists(command: &str) -> bool {
    Command::new("which")
        .arg(command)
        .output()
        .is_ok_and(|output| output.status.success())
}

fn shell_command(command: &str) -> Command {
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
) -> ShellOutput {
    ShellOutput {
        command: command.into(),
        cwd,
        mode: mode.into(),
        status: output.status.code(),
        success: output.status.success(),
        sandboxed,
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
}

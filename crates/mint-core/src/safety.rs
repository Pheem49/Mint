use std::{
    path::{Component, Path, PathBuf},
    sync::LazyLock,
};

use regex::Regex;
use serde::Serialize;
use thiserror::Error;

use crate::MintConfig;

static BLOCKED_COMMAND_PATTERNS: LazyLock<Vec<(Regex, &'static str)>> = LazyLock::new(|| {
    [
        (
            r"\brm\s+(-[^\s]*r[^\s]*f|-rf|-fr)\b",
            "recursive force delete",
        ),
        (r"\bgit\s+reset\s+--hard\b", "destructive git reset"),
        (
            r"\bgit\s+checkout\s+--\b",
            "destructive git checkout path restore",
        ),
        (r"\bgit\s+clean\b.*\s-[^\s]*f", "destructive git clean"),
        (r"\bmkfs(?:\.\w+)?\b", "filesystem formatting"),
        (r"\bdd\s+.*\bof=/dev/", "raw disk write"),
        (
            r">\s*/dev/(?:sd|nvme|hd|mapper)",
            "write redirection to block device",
        ),
        (
            r"\b(shutdown|reboot|poweroff|halt)\b",
            "system power command",
        ),
        (r"\bsudo\b", "privilege escalation"),
        (r"\bchmod\s+-R\s+777\b", "unsafe recursive permissions"),
        (r"\bchown\s+-R\b", "unsafe recursive ownership change"),
        (
            r"\b(curl|wget)\b.*\|\s*(sh|bash|zsh)\b",
            "remote script piping",
        ),
    ]
    .into_iter()
    .map(|(pattern, reason)| (Regex::new(pattern).unwrap(), reason))
    .collect()
});

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum SafetyTier {
    Approval,
    Blocked,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum ShellCommandMode {
    ReadOnly,
    Test,
    Network,
    Mutating,
}

impl ShellCommandMode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::ReadOnly => "readOnly",
            Self::Test => "test",
            Self::Network => "network",
            Self::Mutating => "mutating",
        }
    }
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct ShellClassification {
    pub tier: SafetyTier,
    pub mode: ShellCommandMode,
    pub reason: String,
}

#[derive(Debug, Clone, Copy)]
pub enum Capability {
    Read,
    Write,
}

impl Capability {
    fn as_str(self) -> &'static str {
        match self {
            Self::Read => "read",
            Self::Write => "write",
        }
    }
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum SafetyError {
    #[error("target path is required")]
    TargetPathRequired,
    #[error("blocked {capability} access to sensitive file name: {file_name}")]
    BlockedFileName {
        capability: &'static str,
        file_name: String,
    },
    #[error("blocked {capability} access to protected path: {path}")]
    BlockedPath {
        capability: &'static str,
        path: PathBuf,
    },
    #[error("path {capability} denied by capability policy: {path}")]
    PathDenied {
        capability: &'static str,
        path: PathBuf,
    },
}

pub fn classify_shell_command(command: &str) -> ShellClassification {
    let normalized = command.split_whitespace().collect::<Vec<_>>().join(" ");
    if normalized.is_empty() {
        return ShellClassification {
            tier: SafetyTier::Blocked,
            mode: ShellCommandMode::Mutating,
            reason: "empty shell command".into(),
        };
    }
    for (pattern, reason) in BLOCKED_COMMAND_PATTERNS.iter() {
        if pattern.is_match(&normalized) {
            return ShellClassification {
                tier: SafetyTier::Blocked,
                mode: ShellCommandMode::Mutating,
                reason: (*reason).into(),
            };
        }
    }
    let mode = classify_shell_mode(&normalized);
    ShellClassification {
        tier: SafetyTier::Approval,
        mode,
        reason: format!("{} shell command requires approval", mode.as_str()),
    }
}

pub fn shell_mode_allowed(config: &MintConfig, mode: ShellCommandMode) -> bool {
    config
        .extra
        .get("allowedShellModes")
        .and_then(|value| value.as_array())
        .map(|values| {
            values
                .iter()
                .filter_map(|value| value.as_str())
                .any(|value| value == "*" || value == mode.as_str())
        })
        .unwrap_or(false)
}

fn classify_shell_mode(command: &str) -> ShellCommandMode {
    let lower = command.to_ascii_lowercase();
    if contains_network_command(&lower) {
        return ShellCommandMode::Network;
    }
    if is_test_command(&lower) {
        return ShellCommandMode::Test;
    }
    if is_read_only_command(&lower) {
        return ShellCommandMode::ReadOnly;
    }
    ShellCommandMode::Mutating
}

fn contains_network_command(command: &str) -> bool {
    [
        "curl ",
        "wget ",
        "git clone",
        "npm install",
        "npm ci",
        "pnpm install",
        "pnpm add",
        "yarn add",
        "pip install",
        "cargo install",
        "go install",
        "docker pull",
    ]
    .iter()
    .any(|needle| command.contains(needle))
}

fn is_test_command(command: &str) -> bool {
    [
        "cargo test",
        "cargo check",
        "cargo clippy",
        "cargo fmt",
        "npm test",
        "npm run test",
        "npm run -s test",
        "npm run build",
        "npm run -s build",
        "npm run lint",
        "npm run -s lint",
        "npm run check",
        "npm run -s check",
        "npm run typecheck",
        "npm run -s typecheck",
        "pnpm test",
        "pnpm run test",
        "pnpm run build",
        "yarn test",
        "yarn build",
        "pytest",
        "go test",
    ]
    .iter()
    .any(|prefix| command == *prefix || command.starts_with(&format!("{prefix} ")))
}

fn is_read_only_command(command: &str) -> bool {
    if command.contains('>') || command.contains(" tee ") {
        return false;
    }
    command
        .split([';', '&', '|'])
        .map(str::trim)
        .filter(|segment| !segment.is_empty())
        .all(|segment| {
            let mut words = segment.split_whitespace();
            matches!(
                words.next(),
                Some(
                    "cat"
                        | "cd"
                        | "du"
                        | "find"
                        | "git"
                        | "head"
                        | "ls"
                        | "nl"
                        | "pwd"
                        | "rg"
                        | "sed"
                        | "tail"
                        | "tree"
                        | "wc"
                        | "which"
                )
            ) && !segment.starts_with("git ")
                || is_read_only_git(segment)
        })
}

fn is_read_only_git(segment: &str) -> bool {
    let mut words = segment.split_whitespace();
    if words.next() != Some("git") {
        return false;
    }
    matches!(
        words.next(),
        Some("branch" | "diff" | "log" | "show" | "status")
    )
}

pub fn assert_path_capability(
    target_path: &Path,
    capability: Capability,
    config: &MintConfig,
) -> Result<PathBuf, SafetyError> {
    if target_path.as_os_str().is_empty() {
        return Err(SafetyError::TargetPathRequired);
    }
    let resolved = resolve_path(target_path);
    if !config.safety_enabled {
        return Ok(resolved);
    }
    if let Some(file_name) = resolved.file_name().and_then(|value| value.to_str())
        && config
            .blocked_file_names
            .iter()
            .any(|item| item == file_name)
    {
        return Err(SafetyError::BlockedFileName {
            capability: capability.as_str(),
            file_name: file_name.into(),
        });
    }
    if config
        .blocked_paths
        .iter()
        .map(|path| resolve_path(path))
        .any(|path| resolved.starts_with(path))
    {
        return Err(SafetyError::BlockedPath {
            capability: capability.as_str(),
            path: resolved,
        });
    }
    let allowed_paths = match capability {
        Capability::Read => &config.allowed_read_paths,
        Capability::Write => &config.allowed_write_paths,
    };
    if !allowed_paths
        .iter()
        .map(|path| resolve_path(path))
        .any(|path| resolved.starts_with(path))
    {
        return Err(SafetyError::PathDenied {
            capability: capability.as_str(),
            path: resolved,
        });
    }
    Ok(resolved)
}

fn resolve_path(path: &Path) -> PathBuf {
    let expanded = expand_home(path);
    let absolute = if expanded.is_absolute() {
        expanded
    } else {
        std::env::current_dir()
            .unwrap_or_else(|_| PathBuf::from("."))
            .join(expanded)
    };
    normalize_path(&absolute)
}

fn expand_home(path: &Path) -> PathBuf {
    let value = path.to_string_lossy();
    if value == "~" {
        return dirs::home_dir().unwrap_or_else(|| path.to_path_buf());
    }
    if let Some(remainder) = value.strip_prefix("~/")
        && let Some(home) = dirs::home_dir()
    {
        return home.join(remainder);
    }
    path.to_path_buf()
}

fn normalize_path(path: &Path) -> PathBuf {
    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                normalized.pop();
            }
            _ => normalized.push(component),
        }
    }
    normalized
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blocks_destructive_shell_commands() {
        let result = classify_shell_command("git reset --hard HEAD");
        assert_eq!(result.tier, SafetyTier::Blocked);
        assert_eq!(result.reason, "destructive git reset");
    }

    #[test]
    fn shell_commands_require_approval_by_default() {
        let result = classify_shell_command("git status --short");
        assert_eq!(result.tier, SafetyTier::Approval);
        assert_eq!(result.mode, ShellCommandMode::ReadOnly);
    }

    #[test]
    fn classifies_test_and_network_shell_modes() {
        assert_eq!(
            classify_shell_command("cargo test -p mint-core").mode,
            ShellCommandMode::Test
        );
        assert_eq!(
            classify_shell_command("npm install").mode,
            ShellCommandMode::Network
        );
    }

    #[test]
    fn shell_mode_policy_reads_config_allowlist() {
        let config = MintConfig::default();
        assert!(shell_mode_allowed(&config, ShellCommandMode::ReadOnly));
        assert!(shell_mode_allowed(&config, ShellCommandMode::Test));
        assert!(!shell_mode_allowed(&config, ShellCommandMode::Network));
    }

    #[test]
    fn blocks_sensitive_file_names() {
        let config = MintConfig::default();
        let result = assert_path_capability(Path::new(".env"), Capability::Read, &config);
        assert!(matches!(result, Err(SafetyError::BlockedFileName { .. })));
    }

    #[test]
    fn denies_paths_outside_configured_roots() {
        let config = MintConfig {
            allowed_read_paths: vec![PathBuf::from("/tmp/mint")],
            blocked_paths: vec![],
            ..MintConfig::default()
        };
        let result =
            assert_path_capability(Path::new("/var/lib/private"), Capability::Read, &config);
        assert!(matches!(result, Err(SafetyError::PathDenied { .. })));
    }
}

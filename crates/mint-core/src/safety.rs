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

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct ShellClassification {
    pub tier: SafetyTier,
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
            reason: "empty shell command".into(),
        };
    }
    for (pattern, reason) in BLOCKED_COMMAND_PATTERNS.iter() {
        if pattern.is_match(&normalized) {
            return ShellClassification {
                tier: SafetyTier::Blocked,
                reason: (*reason).into(),
            };
        }
    }
    ShellClassification {
        tier: SafetyTier::Approval,
        reason: "shell command requires approval".into(),
    }
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

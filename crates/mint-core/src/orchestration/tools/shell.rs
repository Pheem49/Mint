use std::path::Path;

use super::super::*;

/// Catch ordinary URL launcher commands, not arbitrary shell programs. This is
/// a workflow guard, not a shell security boundary.
fn launches_external_url(command: &str) -> bool {
    let Some(tokens) = shlex::split(command) else {
        return false;
    };
    let Some(program) = tokens.first() else {
        return false;
    };
    let program = program.rsplit(['/', '\\']).next().unwrap_or(program);
    let launcher = match program.to_ascii_lowercase().as_str() {
        "xdg-open" | "open" | "wslview" | "sensible-browser" | "start-process" | "start" => true,
        "gio" => tokens.get(1).is_some_and(|arg| arg == "open"),
        _ => false,
    };
    launcher
        && tokens.iter().skip(1).any(|arg| {
            reqwest::Url::parse(arg).is_ok_and(|url| {
                matches!(url.scheme(), "http" | "https") && url.host_str().is_some()
            })
        })
}

pub(in crate::orchestration) async fn prevent_duplicate_browser_launch(
    command: &str,
) -> Result<(), OrchestrationError> {
    if launches_external_url(command) && crate::browser::session_finish_evidence().await {
        return Err(OrchestrationError::Agent(
            "External URL launch skipped: this task already uses the automation browser. Use browser_observe/browser_read to inspect its current page, browser_screenshot for visual inspection, or browser_open if navigation is still needed. Do not open the same URL in another browser.".into(),
        ));
    }
    Ok(())
}

/// Handles the subset of `execute_tool` actions related to shell.
/// Only called for actions `execute_tool` has already routed here, so the
/// fallback arm is unreachable in practice.
pub(in crate::orchestration) async fn execute(
    action: &str,
    input: &AgentInput,
    root: &Path,
    config: &MintConfig,
    chat_id: &str,
    approve_cb: &mut (dyn FnMut(&AgentApproval) -> Result<ApprovalOutcome, String> + Send),
) -> Result<String, OrchestrationError> {
    match action {
        "run_shell" => {
            let command = required(&input.command, "command")?;
            prevent_duplicate_browser_launch(command).await?;
            let mode = classify_shell_command(command).mode.as_str().to_owned();
            let approved = approve_cb(&AgentApproval::RunShell {
                command: command.to_owned(),
                mode,
                background: input.background,
            })
            .map_err(OrchestrationError::Agent)?;

            match approved {
                ApprovalOutcome::Approved if input.background => {
                    let started = crate::bg_shell::start_background(root, config, command)
                        .map_err(|e| OrchestrationError::Agent(e.to_string()))?;
                    Ok(format!(
                        "job_id: {}\npid: {}\nstatus: running\nUse the 'shell_output' tool with this job_id to check on it, and 'kill_shell' to stop it.",
                        started.id,
                        started
                            .pid
                            .map_or_else(|| "unknown".to_string(), |p| p.to_string()),
                    ))
                }
                ApprovalOutcome::Approved => run_shell(root, config, chat_id, command).await,
                ApprovalOutcome::Denied => Ok(format!("User denied shell command: {}", command)),
                ApprovalOutcome::Intercepted(obs) => Ok(obs),
            }
        }
        "shell_output" => {
            let job_id = required(&input.job_id, "job_id")?;
            crate::bg_shell::poll_output(job_id)
                .map_err(|e| OrchestrationError::Agent(e.to_string()))
        }
        "kill_shell" => {
            let job_id = required(&input.job_id, "job_id")?;
            crate::bg_shell::kill_job(job_id).map_err(|e| OrchestrationError::Agent(e.to_string()))
        }
        "verify" => {
            if input.commands.is_empty() {
                return Err(OrchestrationError::Agent(
                    "verify requires at least one command".into(),
                ));
            }
            let mut output = Vec::new();
            for command in &input.commands {
                output.push(run_shell(root, config, chat_id, command).await?);
            }
            Ok(output.join("\n\n"))
        }
        "run_tests" => {
            let cmd = if !input.command.trim().is_empty() {
                input.command.trim().to_string()
            } else if root.join("Cargo.toml").exists() {
                if !input.filter.trim().is_empty() {
                    format!("cargo test {}", input.filter.trim())
                } else {
                    "cargo test".to_string()
                }
            } else if root.join("package.json").exists() {
                if !input.filter.trim().is_empty() {
                    format!("npm test -- {}", input.filter.trim())
                } else {
                    "npm test".to_string()
                }
            } else if root.join("pyproject.toml").exists() || root.join("requirements.txt").exists()
            {
                if !input.filter.trim().is_empty() {
                    format!("pytest -k {}", input.filter.trim())
                } else {
                    "pytest".to_string()
                }
            } else if root.join("go.mod").exists() {
                "go test ./...".to_string()
            } else {
                return Err(OrchestrationError::Agent(
                    "No test runner auto-detected. Specify a command via the 'command' argument."
                        .into(),
                ));
            };

            let out = run_shell(root, config, chat_id, &cmd).await?;
            let failed = shell_result_failed(&out);
            let status_mark = if failed {
                "✗ [TEST FAILED]"
            } else {
                "✓ [TEST PASSED]"
            };
            Ok(format!("{}\nCommand: {}\n\n{}", status_mark, cmd, out))
        }
        "run_typecheck" => {
            let cmd = if !input.command.trim().is_empty() {
                input.command.trim().to_string()
            } else if root.join("Cargo.toml").exists() {
                "cargo check".to_string()
            } else if root.join("package.json").exists() {
                if root.join("tsconfig.json").exists() {
                    "npx tsc --noEmit".to_string()
                } else {
                    "npm run build".to_string()
                }
            } else if root.join("pyproject.toml").exists() {
                "python -m compileall .".to_string()
            } else if root.join("go.mod").exists() {
                "go vet ./...".to_string()
            } else {
                return Err(OrchestrationError::Agent(
                    "No typechecker auto-detected. Specify a command via 'command'.".into(),
                ));
            };

            let out = run_shell(root, config, chat_id, &cmd).await?;
            let failed = shell_result_failed(&out);
            let status_mark = if failed {
                "✗ [TYPECHECK FAILED]"
            } else {
                "✓ [TYPECHECK PASSED]"
            };
            Ok(format!("{}\nCommand: {}\n\n{}", status_mark, cmd, out))
        }
        "run_linter" => {
            let cmd = if !input.command.trim().is_empty() {
                input.command.trim().to_string()
            } else if root.join("Cargo.toml").exists() {
                "cargo clippy".to_string()
            } else if root.join("package.json").exists() {
                "npx eslint .".to_string()
            } else if root.join("pyproject.toml").exists() {
                "flake8 .".to_string()
            } else if root.join("go.mod").exists() {
                "golangci-lint run".to_string()
            } else {
                return Err(OrchestrationError::Agent(
                    "No linter auto-detected. Specify a command via 'command'.".into(),
                ));
            };

            let out = run_shell(root, config, chat_id, &cmd).await?;
            let failed = shell_result_failed(&out);
            let status_mark = if failed {
                "✗ [LINT FAILED]"
            } else {
                "✓ [LINT PASSED]"
            };
            Ok(format!("{}\nCommand: {}\n\n{}", status_mark, cmd, out))
        }
        _ => unreachable!(
            "execute_tool routed an unhandled action into tools::shell::execute: {action}"
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn external_url_launch_detection_preserves_other_shell_work() {
        for command in [
            "xdg-open 'https://example.com/watch?v=1&list=2'",
            "/usr/bin/xdg-open https://example.com &",
            "open -a Firefox https://example.com",
            "gio open https://example.com",
            "wslview https://example.com",
            "Start-Process https://example.com",
        ] {
            assert!(launches_external_url(command), "{command}");
        }
        for command in [
            "xdg-open /tmp/report.pdf",
            "open /tmp",
            "curl https://example.com",
            "echo xdg-open https://example.com",
            "gio info https://example.com",
            "cargo test",
        ] {
            assert!(!launches_external_url(command), "{command}");
        }
    }

    #[tokio::test]
    async fn external_url_launch_allowed_without_browser_use() {
        crate::browser::run_session(async {
            prevent_duplicate_browser_launch("xdg-open https://example.com")
                .await
                .unwrap();
        })
        .await;
    }
}

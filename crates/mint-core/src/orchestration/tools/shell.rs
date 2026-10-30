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

/// Deletes a Chrome run's throw-away profile once the tool call is over, however
/// it ends (success, denial, repeat refusal, error). Background jobs outlive the
/// call, so they disarm it and rely on the stale-profile sweep instead.
struct ProfileCleanup<'a>(Option<&'a crate::browser::headless::ChromeRun>);

impl Drop for ProfileCleanup<'_> {
    fn drop(&mut self) {
        if let Some(run) = self.0 {
            run.discard_profile();
        }
    }
}

pub(in crate::orchestration) async fn prevent_duplicate_browser_launch(
    command: &str,
) -> Result<(), OrchestrationError> {
    if launches_external_url(command) && crate::browser::session_finish_evidence().await {
        return Err(OrchestrationError::Agent(
            "External URL launch skipped: this task already uses an active browser session. Do not open the same URL in another browser.".into(),
        ));
    }
    Ok(())
}

/// Refuses to capture the same page with the same options twice in one turn:
/// that is the screenshot → view_image → recapture loop. A `retryReason` is the
/// model's explicit statement that the page changed, and a deleted capture file
/// may always be retaken.
fn repeat_capture_notice(
    run: &crate::browser::headless::ChromeRun,
    root: &Path,
    retry_reason: &str,
) -> Option<String> {
    if !retry_reason.trim().is_empty() {
        return None;
    }
    let previous = reliability::recorded_capture(run.key.as_ref()?.as_str())?;
    match previous {
        Some(file) if root.join(&file).exists() => Some(format!(
            "Already captured: this page was captured with the same options earlier in this turn at {file}. \
             Call view_image on that file once (or read it) instead of capturing again, or pass \
             retryReason if the page must be fetched again."
        )),
        Some(_) => None,
        None => Some(
            "Already read: this page was read with the same options earlier in this turn. Reuse that \
             output, change the options (for example capture a screenshot), or pass retryReason if the \
             page must be fetched again."
                .into(),
        ),
    }
}

/// Remembers a successful capture so `repeat_capture_notice` can spot repeats.
fn record_capture_result(run: &crate::browser::headless::ChromeRun, result: &str) {
    if let Some(key) = &run.key
        && !result.starts_with("Skipped")
        && !shell_result_failed(result)
    {
        reliability::record_capture(key.as_str(), run.output.clone());
    }
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
            // Headless Chrome is made reliable (flags, throw-away profile, timeout) and
            // de-duplicated here, so the model only has to write the plain command.
            let chrome = crate::browser::headless::prepare(command, config);
            let mut cleanup = ProfileCleanup(chrome.as_ref());
            let command = chrome.as_ref().map_or(command, |run| run.command.as_str());
            if let Some(run) = &chrome
                && let Some(notice) = repeat_capture_notice(run, root, &input.retry_reason)
            {
                return Ok(notice);
            }
            let mode = classify_shell_command(command).mode.as_str().to_owned();
            let approved = approve_cb(&AgentApproval::RunShell {
                command: command.to_owned(),
                mode,
                background: input.background,
            })
            .map_err(OrchestrationError::Agent)?;

            match approved {
                ApprovalOutcome::Approved if input.background => {
                    if let Err(reason) = reliability::before_shell(command, &input.retry_reason) {
                        return Ok(format!("Skipped: {reason}"));
                    }
                    let started = crate::bg_shell::with_context(
                        crate::bg_shell::chat_context(root, chat_id),
                        || crate::bg_shell::start_background(root, config, command),
                    )
                    .map_err(|e| OrchestrationError::Agent(e.to_string()))?;
                    cleanup.0 = None;
                    reliability::record(
                        command,
                        reliability::ExecutionState::Running,
                        None,
                        Some(started.id.clone()),
                        "",
                    );
                    Ok(format!(
                        "job_id: {}\npid: {}\nstatus: running\nUse the 'shell_output' tool with this job_id to check on it, and 'kill_shell' to stop it.",
                        started.id,
                        started
                            .pid
                            .map_or_else(|| "unknown".to_string(), |p| p.to_string()),
                    ))
                }
                ApprovalOutcome::Approved => {
                    let out = run_shell(root, config, chat_id, command, &input.retry_reason).await?;
                    if let Some(run) = &chrome {
                        record_capture_result(run, &out);
                    }
                    Ok(out)
                }
                ApprovalOutcome::Denied => Ok(format!("User denied shell command: {}", command)),
                ApprovalOutcome::Intercepted(obs) => Ok(obs),
            }
        }
        "shell_output" => {
            let job_id = required(&input.job_id, "job_id")?;
            if !crate::bg_shell::accessible(
                job_id,
                &crate::bg_shell::chat_context(root, chat_id).owner,
            ) {
                return Err(OrchestrationError::Agent(
                    "No accessible background job with that id".into(),
                ));
            }
            let snapshot = crate::bg_shell::snapshot(job_id)
                .map_err(|e| OrchestrationError::Agent(e.to_string()))?;
            reliability::record_job(&snapshot);
            crate::bg_shell::poll_output(job_id)
                .map_err(|e| OrchestrationError::Agent(e.to_string()))
        }
        "kill_shell" => {
            let job_id = required(&input.job_id, "job_id")?;
            if !crate::bg_shell::accessible(
                job_id,
                &crate::bg_shell::chat_context(root, chat_id).owner,
            ) {
                return Err(OrchestrationError::Agent(
                    "No accessible background job with that id".into(),
                ));
            }
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
                output.push(run_shell(root, config, chat_id, command, &input.retry_reason).await?);
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

            let out = run_shell(root, config, chat_id, &cmd, &input.retry_reason).await?;
            if out.starts_with("Skipped") {
                return Ok(out);
            }
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

            let out = run_shell(root, config, chat_id, &cmd, &input.retry_reason).await?;
            if out.starts_with("Skipped") {
                return Ok(out);
            }
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

            let out = run_shell(root, config, chat_id, &cmd, &input.retry_reason).await?;
            if out.starts_with("Skipped") {
                return Ok(out);
            }
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
    use crate::browser::headless::{HostEnv, prepare_with};

    fn chrome(command: &str) -> crate::browser::headless::ChromeRun {
        let host = HostEnv {
            no_sandbox: false,
            linux_timeout: false,
            profile_root: std::env::temp_dir().join("mint-headless-shell-test"),
        };
        prepare_with(command, &MintConfig::default(), &host).unwrap()
    }

    #[tokio::test]
    async fn a_repeated_capture_of_the_same_page_is_refused_until_a_reason_is_given() {
        let dir = std::env::temp_dir().join(format!("mint-capture-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        reliability::scope(async {
            let first = chrome("google-chrome --screenshot=a.png https://example.com/shop");
            assert!(repeat_capture_notice(&first, &dir, "").is_none(), "first capture runs");
            std::fs::write(dir.join("a.png"), b"png").unwrap();
            record_capture_result(&first, "");

            let again = chrome("google-chrome --screenshot=b.png https://example.com/shop/");
            let notice = repeat_capture_notice(&again, &dir, "").expect("repeat refused");
            assert!(notice.contains("a.png") && notice.contains("view_image"), "{notice}");
            assert!(
                repeat_capture_notice(&again, &dir, "page changed after login").is_none(),
                "a retryReason allows a deliberate recapture"
            );
            std::fs::remove_file(dir.join("a.png")).unwrap();
            assert!(
                repeat_capture_notice(&again, &dir, "").is_none(),
                "a deleted capture may be retaken"
            );
        })
        .await;
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn dom_reads_repeat_detection_and_failed_runs_are_not_recorded() {
        let dir = std::env::temp_dir();
        reliability::scope(async {
            let dom = chrome("google-chrome --dump-dom https://example.com");
            record_capture_result(&dom, "Error: Chrome failed\nexit: 1\n");
            assert!(repeat_capture_notice(&dom, &dir, "").is_none(), "failures can be retried");
            record_capture_result(&dom, "<html></html>");
            let notice = repeat_capture_notice(&dom, &dir, "").expect("repeat refused");
            assert!(notice.starts_with("Already read"), "{notice}");
        })
        .await;
    }

    #[test]
    fn commands_without_a_single_url_are_never_deduplicated() {
        let run = chrome("google-chrome --dump-dom");
        assert!(repeat_capture_notice(&run, &std::env::temp_dir(), "").is_none());
    }

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

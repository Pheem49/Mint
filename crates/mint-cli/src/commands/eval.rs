use std::path::PathBuf;
use std::time::Instant;
use anyhow::{Context, Result};
use mint_core::MintConfig;
use mint_core::eval::{
    evaluate_task_result, aggregate_benchmark_report, load_suite_from_file,
};

pub async fn handle_eval(suite_path: PathBuf, limit: usize, config: &MintConfig) -> Result<()> {
    println!("\x1b[1;36m┌─ Mint Benchmark Evaluation Harness ──────────────────────\x1b[0m");
    if !suite_path.exists() {
        println!("│ \x1b[31mBenchmark suite file not found: {}\x1b[0m", suite_path.display());
        println!("\x1b[1;36m└─────────────────────────────────────────────────────────\x1b[0m");
        anyhow::bail!("Suite file not found: {}", suite_path.display());
    }

    let suite = load_suite_from_file(&suite_path)
        .map_err(|e| anyhow::anyhow!("{e}"))?;

    println!("│ Suite:       \x1b[1m{}\x1b[0m (v{})", suite.name, suite.version);
    println!("│ Total Tasks: {}", suite.tasks.len());
    println!("\x1b[1;36m├─────────────────────────────────────────────────────────\x1b[0m");

    let cwd = std::env::current_dir().context("failed to get current dir")?;
    let mut results = Vec::new();
    let tasks_to_run: Vec<_> = suite.tasks.into_iter().take(limit.max(1)).collect();
    let run_count = tasks_to_run.len();

    for (idx, task) in tasks_to_run.into_iter().enumerate() {
        println!("│ [{}/{}] Task: \x1b[1;33m{}\x1b[0m ({})", idx + 1, run_count, task.title, task.id);
        let start = Instant::now();

        // 1. Setup commands
        for cmd in &task.setup_commands {
            println!("│   Running setup: {cmd}");
            let _ = std::process::Command::new("sh")
                .arg("-c")
                .arg(cmd)
                .current_dir(&cwd)
                .status();
        }

        // 2. Execute agent task
        println!("│   Executing agent task: \"{}\"...", task.prompt);
        let agent_res = crate::agent::run_code_agent(&task.prompt, &cwd, config).await;
        let duration = start.elapsed().as_secs_f64();

        // 3. Verification commands
        let mut test_passed = true;
        for verify_cmd in &task.verify_commands {
            println!("│   Running verify: {verify_cmd}");
            let status = std::process::Command::new("sh")
                .arg("-c")
                .arg(verify_cmd)
                .current_dir(&cwd)
                .status();
            match status {
                Ok(s) if s.success() => {},
                _ => {
                    test_passed = false;
                }
            }
        }

        let eval_res = match agent_res {
            Ok(res) => {
                let files_modified: Vec<String> = std::process::Command::new("git")
                    .args(["status", "--porcelain"])
                    .current_dir(&cwd)
                    .output()
                    .ok()
                    .map(|o| {
                        String::from_utf8_lossy(&o.stdout)
                            .lines()
                            .filter_map(|l| {
                                let trimmed = l.trim();
                                if trimmed.len() > 3 {
                                    Some(trimmed[3..].trim().to_string())
                                } else {
                                    None
                                }
                            })
                            .collect()
                    })
                    .unwrap_or_default();

                evaluate_task_result(
                    &task,
                    res.total_tokens as usize,
                    1,
                    &files_modified,
                    duration,
                    0,
                    test_passed,
                    true,
                    None,
                )
            }
            Err(e) => {
                evaluate_task_result(
                    &task,
                    0,
                    0,
                    &[],
                    duration,
                    0,
                    false,
                    false,
                    Some(e.to_string()),
                )
            }
        };

        let status_mark = if eval_res.passed {
            "\x1b[32m[PASS]\x1b[0m"
        } else {
            "\x1b[31m[FAIL]\x1b[0m"
        };
        println!(
            "│   Outcome: {} in {:.1}s (Tokens: {})",
            status_mark, eval_res.duration_secs, eval_res.total_tokens
        );
        results.push(eval_res);
    }

    let report = aggregate_benchmark_report(&suite.name, results);
    println!("\x1b[1;36m├─ Final Evaluation Report ───────────────────────────────\x1b[0m");
    println!("│ Suite:            {}", report.suite_name);
    println!("│ Tasks Completed:  {}/{}", report.passed_tasks, report.total_tasks);
    println!("│ Success Rate:     \x1b[1;{}m{:.1}%\x1b[0m", if report.success_rate >= 80.0 { 32 } else { 33 }, report.success_rate);
    println!("│ Avg Duration:     {:.1}s", report.avg_duration_secs);
    println!("│ Avg Tokens:       {}", report.avg_tokens);
    println!("│ Avg Tool Calls:   {:.1}", report.avg_tool_calls);
    println!("│ Avg Retries:      {:.1}", report.avg_retries);
    println!("\x1b[1;36m└─────────────────────────────────────────────────────────\x1b[0m");

    let report_path = cwd.join("mint_eval_report.json");
    if let Ok(json) = serde_json::to_string_pretty(&report) {
        let _ = std::fs::write(&report_path, json);
        println!("Report saved to: {}", report_path.display());
    }

    Ok(())
}

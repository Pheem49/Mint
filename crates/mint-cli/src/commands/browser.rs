use anyhow::{Context, Result};
use clap::Subcommand;
use mint_core::{Capability, MintConfig};
use std::path::PathBuf;

#[derive(Debug, Subcommand)]
pub enum BrowserCommand {
    /// Run a JSON workflow in one isolated headless browser session.
    Run {
        #[arg(long)]
        file: PathBuf,
    },
}

pub async fn handle(command: BrowserCommand, config: &MintConfig, plan_mode: bool) -> Result<()> {
    let BrowserCommand::Run { file } = command;
    let mut config = config.clone();
    if let Ok(policy) = std::env::var("MINT_BROWSER_POLICY") {
        let policy: serde_json::Value =
            serde_json::from_str(&policy).context("Invalid parent browser policy")?;
        if let Some(disabled) = policy["disabledTools"].as_array() {
            for tool in disabled.iter().filter_map(|s| s.as_str()) {
                if !config.disabled_tools.iter().any(|t| t == tool) {
                    config.disabled_tools.push(tool.into());
                }
            }
        }
        for key in [
            "browserExecutable",
            "browserViewportWidth",
            "browserViewportHeight",
            "browserLaunchArgs",
        ] {
            if !policy[key].is_null() {
                config.extra.insert(key.into(), policy[key].clone());
            }
        }
        if let Some(safety) = policy["safetyEnabled"].as_bool() {
            config.safety_enabled = safety;
        }
        if let Some(paths) = policy.get("allowedReadPaths") {
            config.allowed_read_paths = serde_json::from_value(paths.clone())?;
        }
        if let Some(paths) = policy.get("allowedWritePaths") {
            config.allowed_write_paths = serde_json::from_value(paths.clone())?;
        }
    }
    let file = mint_core::assert_path_capability(&file, Capability::Read, &config)?;
    anyhow::ensure!(
        std::fs::metadata(&file)?.len() <= 1_048_576,
        "Workflow file exceeds 1 MiB"
    );
    let workflow: mint_core::browser::workflow::BrowserWorkflow =
        serde_json::from_slice(&std::fs::read(file)?)?;
    mint_core::browser::workflow::validate(&workflow, &config, plan_mode)
        .map_err(anyhow::Error::msg)?;
    let root =
        mint_core::assert_path_capability(&std::env::current_dir()?, Capability::Write, &config)?;
    let report_path = std::env::var_os("MINT_BROWSER_REPORT")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            root.join(".mint/browser-runs")
                .join(format!(
                    "{}",
                    chrono::Utc::now().timestamp_nanos_opt().unwrap_or_default()
                ))
                .join("report.json")
        });
    let output = report_path.parent().context("Invalid report path")?;
    anyhow::ensure!(
        output.starts_with(&root),
        "Browser output must be inside the workflow workspace"
    );
    std::fs::create_dir_all(output)?;
    let output = output.canonicalize()?;
    anyhow::ensure!(
        output.starts_with(root.canonicalize()?),
        "Browser output resolves outside the workspace"
    );
    let run = mint_core::browser::workflow::run(&config, workflow, &output, plan_mode, |message| {
        eprintln!("{message}")
    });
    let report = tokio::select! {
        result=run=>result.map_err(anyhow::Error::msg)?,
        _=cancellation_signal()=>anyhow::bail!("Browser workflow cancelled; input completion may be unconfirmed"),
    };
    let encoded = serde_json::to_vec_pretty(&report)?;
    std::fs::write(output.join("report.json"), &encoded)?;
    println!("{}", String::from_utf8(encoded)?);
    anyhow::ensure!(
        report.status == "succeeded",
        "Browser workflow {}: {}",
        report.status,
        report.error.as_deref().unwrap_or("Inspect the report")
    );
    Ok(())
}

async fn cancellation_signal() {
    #[cfg(unix)]
    {
        if let Ok(mut terminate) =
            tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
        {
            tokio::select! { _=tokio::signal::ctrl_c()=>{}, _=terminate.recv()=>{} }
        } else {
            let _ = tokio::signal::ctrl_c().await;
        }
    }
    #[cfg(not(unix))]
    {
        let _ = tokio::signal::ctrl_c().await;
    }
}

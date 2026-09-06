use std::path::PathBuf;

use anyhow::Result;
use clap::Subcommand;
use mint_core::{CHAT_CLI_ID, CronStore, MemoryStore, MintConfig, TaskStore};

use crate::{agent, cron_wizard};

#[derive(Debug, Subcommand)]
pub enum MemoryCommand {
    /// Read one profile value.
    Get { key: String },
    /// Store one profile value.
    Set { key: String, value: String },
    /// Show recent chat interactions.
    Recent {
        #[arg(long, default_value_t = 5)]
        limit: usize,
    },
}

#[derive(Debug, Subcommand)]
pub enum TaskCommand {
    Add {
        description: String,
    },
    List,
    Show {
        id: String,
    },
    Pending,
    Resume,
    Update {
        id: String,
        status: String,
        #[arg(long)]
        result: Option<String>,
    },
    ClearCompleted,
}

#[derive(Debug, Subcommand)]
pub enum CronCommand {
    /// Create a new scheduled job.
    Add {
        #[arg(long)]
        name: Option<String>,
        #[arg(long)]
        schedule: Option<String>,
        #[arg(long)]
        task: Option<String>,
        #[arg(long)]
        workspace: Option<PathBuf>,
        /// IANA timezone name (e.g. "Asia/Bangkok") to interpret `schedule` in.
        #[arg(long)]
        timezone: Option<String>,
    },
    List,
    Show {
        id: String,
    },
    Remove {
        id: String,
    },
    Enable {
        id: String,
    },
    Disable {
        id: String,
    },
    /// Run a job immediately, without waiting for its schedule.
    RunNow {
        id: String,
    },
}

pub fn handle_memory(command: MemoryCommand) -> Result<()> {
    let memory = MemoryStore::open_default()?;
    match command {
        MemoryCommand::Get { key } => {
            println!("{}", memory.get_profile(&key)?.unwrap_or_default());
        }
        MemoryCommand::Set { key, value } => {
            memory.set_profile(&key, &value)?;
            println!("stored");
        }
        MemoryCommand::Recent { limit } => {
            let current_dir = std::env::current_dir()?;
            let scoped_chat_id =
                mint_core::scoped_chat_id(CHAT_CLI_ID, Some(&current_dir.to_string_lossy()));
            println!(
                "{}",
                serde_json::to_string_pretty(
                    &memory.recent_interactions_for_chat(&scoped_chat_id, limit)?
                )?
            );
        }
    }
    Ok(())
}

pub fn handle_task(command: TaskCommand) -> Result<()> {
    let tasks = TaskStore::open_default()?;
    match command {
        TaskCommand::Add { description } => {
            println!(
                "{}",
                serde_json::to_string_pretty(&tasks.add(description)?)?
            );
        }
        TaskCommand::List => {
            println!("{}", serde_json::to_string_pretty(&tasks.list()?)?)
        }
        TaskCommand::Show { id } => {
            println!("{}", serde_json::to_string_pretty(&tasks.get(&id)?)?)
        }
        TaskCommand::Pending => {
            println!("{}", serde_json::to_string_pretty(&tasks.pending()?)?)
        }
        TaskCommand::Resume => {
            println!(
                "{}",
                serde_json::to_string_pretty(&tasks.resume_running()?)?
            )
        }
        TaskCommand::Update { id, status, result } => {
            println!(
                "{}",
                serde_json::to_string_pretty(&tasks.update_status(
                    &id,
                    &status,
                    result.map(serde_json::Value::String)
                )?)?
            )
        }
        TaskCommand::ClearCompleted => println!("{}", tasks.clear_completed()?),
    }
    Ok(())
}

pub async fn handle_cron(command: CronCommand, config: &MintConfig) -> Result<()> {
    let cron_jobs = CronStore::open_default()?;
    match command {
        CronCommand::Add {
            name,
            schedule,
            task,
            workspace,
            timezone,
        } => {
            let default_workspace = std::env::current_dir()?;
            if name.is_none() && schedule.is_none() && task.is_none() {
                let job = cron_wizard::run_add_wizard(&cron_jobs, &default_workspace)?;
                println!("\nCreated cron job {} — next run: {}", job.id, job.next_run);
            } else {
                let name = name.ok_or_else(|| anyhow::anyhow!("--name is required"))?;
                let schedule = schedule
                    .ok_or_else(|| anyhow::anyhow!("--schedule is required"))?;
                let task = task.ok_or_else(|| anyhow::anyhow!("--task is required"))?;
                let workspace = workspace.unwrap_or(default_workspace);
                let schedule = match timezone {
                    Some(tz) => {
                        mint_core::localize_schedule(&schedule, &tz, chrono::Utc::now())
                            .map_err(|e| anyhow::anyhow!(e))?
                    }
                    None => schedule,
                };
                println!(
                    "{}",
                    serde_json::to_string_pretty(
                        &cron_jobs.add(name, schedule, task, workspace)?
                    )?
                );
            }
        }
        CronCommand::List => {
            println!("{}", serde_json::to_string_pretty(&cron_jobs.list()?)?)
        }
        CronCommand::Show { id } => {
            println!("{}", serde_json::to_string_pretty(&cron_jobs.get(&id)?)?)
        }
        CronCommand::Remove { id } => {
            println!("{}", serde_json::to_string_pretty(&cron_jobs.remove(&id)?)?)
        }
        CronCommand::Enable { id } => {
            println!(
                "{}",
                serde_json::to_string_pretty(&cron_jobs.set_enabled(&id, true)?)?
            )
        }
        CronCommand::Disable { id } => {
            println!(
                "{}",
                serde_json::to_string_pretty(&cron_jobs.set_enabled(&id, false)?)?
            )
        }
        CronCommand::RunNow { id } => {
            let job = cron_jobs
                .get(&id)?
                .ok_or_else(|| anyhow::anyhow!("no cron job with id {id}"))?;
            println!("Running cron job {}: {}", job.id, job.name);
            match agent::run_code_agent(&job.task, &job.workspace, config).await {
                Ok(result) => {
                    cron_jobs.record_run(
                        &job.id,
                        "success",
                        Some(result.summary.clone()),
                    )?;
                    println!("Job completed: {}", result.summary);
                }
                Err(error) => {
                    cron_jobs.record_run(&job.id, "failed", Some(error.to_string()))?;
                    return Err(error);
                }
            }
        }
    }
    Ok(())
}

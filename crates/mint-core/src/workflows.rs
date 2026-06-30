use std::{fs, path::PathBuf};

use serde_json::{Value, json};
use thiserror::Error;

use crate::{ConfigError, config_path};

#[derive(Debug, Error)]
pub enum WorkflowError {
    #[error(transparent)]
    Config(#[from] ConfigError),
    #[error("workflow directory is unavailable")]
    MissingDirectory,
    #[error("unable to create workflow directory: {0}")]
    CreateDirectory(std::io::Error),
    #[error("unable to read {path}: {source}")]
    Read {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("unable to parse {path}: {source}")]
    Parse {
        path: PathBuf,
        source: serde_json::Error,
    },
    #[error("unable to serialize workflows: {0}")]
    Serialize(serde_json::Error),
    #[error("unable to write {path}: {source}")]
    Write {
        path: PathBuf,
        source: std::io::Error,
    },
}

pub fn workflows_path() -> Result<PathBuf, WorkflowError> {
    Ok(config_path()?.with_file_name("workflows.json"))
}

pub fn load_workflows() -> Result<Vec<Value>, WorkflowError> {
    let path = workflows_path()?;
    if !path.exists() {
        save_default_workflows(&path)?;
    }
    let raw = fs::read_to_string(&path).map_err(|source| WorkflowError::Read {
        path: path.clone(),
        source,
    })?;
    serde_json::from_str(&raw).map_err(|source| WorkflowError::Parse { path, source })
}

fn save_default_workflows(path: &PathBuf) -> Result<(), WorkflowError> {
    let directory = path.parent().ok_or(WorkflowError::MissingDirectory)?;
    fs::create_dir_all(directory).map_err(WorkflowError::CreateDirectory)?;
    let workflows = json!([
        {
            "id": "wf-1",
            "name": "Check Mic on Zoom",
            "trigger": { "type": "process_running", "processName": "zoom" },
            "action": {
                "type": "system_info",
                "message": "Looks like you opened Zoom. Should I check your system resources?",
                "target": ""
            }
        },
        {
            "id": "wf-2",
            "name": "Coding Time",
            "trigger": { "type": "process_running", "processName": "code" },
            "action": {
                "type": "open_app",
                "message": "Coding time. Want me to open Spotify?",
                "target": "spotify"
            }
        }
    ]);
    let raw = serde_json::to_string_pretty(&workflows).map_err(WorkflowError::Serialize)?;
    fs::write(path, format!("{raw}\n")).map_err(|source| WorkflowError::Write {
        path: path.clone(),
        source,
    })
}

pub fn save_workflows(workflows: &[Value]) -> Result<(), WorkflowError> {
    let path = workflows_path()?;
    let directory = path.parent().ok_or(WorkflowError::MissingDirectory)?;
    fs::create_dir_all(directory).map_err(WorkflowError::CreateDirectory)?;
    let raw = serde_json::to_string_pretty(workflows).map_err(WorkflowError::Serialize)?;
    fs::write(&path, format!("{raw}\n")).map_err(|source| WorkflowError::Write { path, source })
}

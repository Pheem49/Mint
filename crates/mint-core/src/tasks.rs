use std::{
    fs,
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};

use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum TaskError {
    #[error("unable to determine the user config directory")]
    ConfigDirectoryUnavailable,
    #[error("unable to create task directory {path}: {source}")]
    CreateDirectory {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("unable to read tasks file {path}: {source}")]
    Read {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("unable to parse tasks file {path}: {source}")]
    Parse {
        path: PathBuf,
        source: serde_json::Error,
    },
    #[error("unable to serialize tasks: {0}")]
    Serialize(serde_json::Error),
    #[error("unable to write tasks file {path}: {source}")]
    Write {
        path: PathBuf,
        source: std::io::Error,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Task {
    pub id: String,
    pub description: String,
    pub status: String,
    pub created_at: String,
    pub updated_at: String,
    #[serde(default)]
    pub steps: Vec<Value>,
    #[serde(default)]
    pub subtasks: Vec<Value>,
    #[serde(default)]
    pub checkpoints: Vec<Value>,
    #[serde(default)]
    pub artifacts: Vec<Value>,
    #[serde(default)]
    pub retry_count: u32,
    #[serde(default = "default_max_retries")]
    pub max_retries: u32,
    #[serde(default)]
    pub last_checkpoint_at: Option<String>,
    #[serde(default)]
    pub result: Option<Value>,
    #[serde(flatten)]
    pub extra: serde_json::Map<String, Value>,
}

#[derive(Clone)]
pub struct TaskStore {
    path: PathBuf,
}

impl TaskStore {
    pub fn open_default() -> Result<Self, TaskError> {
        Ok(Self::open(tasks_path()?))
    }

    pub fn open(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }

    pub fn list(&self) -> Result<Vec<Task>, TaskError> {
        if !self.path.exists() {
            return Ok(Vec::new());
        }
        let raw = fs::read_to_string(&self.path).map_err(|source| TaskError::Read {
            path: self.path.clone(),
            source,
        })?;
        serde_json::from_str(&raw).map_err(|source| TaskError::Parse {
            path: self.path.clone(),
            source,
        })
    }

    pub fn add(&self, description: impl Into<String>) -> Result<Task, TaskError> {
        let now = timestamp();
        let task = Task {
            id: now.clone(),
            description: description.into(),
            status: "pending".into(),
            created_at: now.clone(),
            updated_at: now,
            steps: Vec::new(),
            subtasks: Vec::new(),
            checkpoints: Vec::new(),
            artifacts: Vec::new(),
            retry_count: 0,
            max_retries: default_max_retries(),
            last_checkpoint_at: None,
            result: None,
            extra: serde_json::Map::new(),
        };
        let mut tasks = self.list()?;
        tasks.push(task.clone());
        self.write(&tasks)?;
        Ok(task)
    }

    pub fn get(&self, id: &str) -> Result<Option<Task>, TaskError> {
        Ok(self.list()?.into_iter().find(|task| task.id == id))
    }

    pub fn pending(&self) -> Result<Option<Task>, TaskError> {
        Ok(self
            .list()?
            .into_iter()
            .find(|task| task.status == "pending"))
    }

    pub fn update_status(
        &self,
        id: &str,
        status: &str,
        result: Option<Value>,
    ) -> Result<Option<Task>, TaskError> {
        self.mutate(id, |task| {
            task.status = status.into();
            if result.is_some() {
                task.result = result;
            }
        })
    }

    pub fn add_checkpoint(
        &self,
        id: &str,
        mut checkpoint: Value,
    ) -> Result<Option<Task>, TaskError> {
        self.mutate(id, |task| {
            let time = timestamp();
            let checkpoint_id = format!("{id}-checkpoint-{}", task.checkpoints.len() + 1);
            if let Some(value) = checkpoint.as_object_mut() {
                value.entry("id").or_insert(Value::String(checkpoint_id));
                value.entry("time").or_insert(Value::String(time.clone()));
            }
            task.last_checkpoint_at = Some(time);
            task.steps.push(checkpoint.clone());
            task.checkpoints.push(checkpoint);
        })
    }

    pub fn add_artifact(&self, id: &str, mut artifact: Value) -> Result<Option<Task>, TaskError> {
        self.mutate(id, |task| {
            if let Some(value) = artifact.as_object_mut() {
                value.entry("id").or_insert(Value::String(format!(
                    "{id}-artifact-{}",
                    task.artifacts.len() + 1
                )));
                value
                    .entry("time")
                    .or_insert_with(|| Value::String(timestamp()));
            }
            task.artifacts.push(artifact);
        })
    }

    pub fn fail_with_retry(&self, id: &str, message: &str) -> Result<Option<Task>, TaskError> {
        self.mutate(id, |task| {
            task.retry_count += 1;
            task.status = if task.retry_count <= task.max_retries {
                "pending"
            } else {
                "failed"
            }
            .into();
            task.result = Some(Value::String(message.into()));
            let checkpoint = serde_json::json!({
                "id": format!("{id}-checkpoint-{}", task.checkpoints.len() + 1),
                "time": timestamp(),
                "phase": if task.status == "pending" { "retry_scheduled" } else { "failed" },
                "message": message,
                "retryCount": task.retry_count,
                "maxRetries": task.max_retries,
            });
            task.steps.push(checkpoint.clone());
            task.checkpoints.push(checkpoint);
        })
    }

    pub fn resume_running(&self) -> Result<Vec<Task>, TaskError> {
        let mut tasks = self.list()?;
        let mut resumed = Vec::new();
        for task in &mut tasks {
            if task.status != "running" {
                continue;
            }
            task.status = "pending".into();
            task.updated_at = timestamp();
            let checkpoint = serde_json::json!({
                "id": format!("{}-checkpoint-{}", task.id, task.checkpoints.len() + 1),
                "time": timestamp(),
                "phase": "resume_after_restart",
                "message": "Task was running during shutdown and has been re-queued.",
            });
            task.steps.push(checkpoint.clone());
            task.checkpoints.push(checkpoint);
            resumed.push(task.clone());
        }
        self.write(&tasks)?;
        Ok(resumed)
    }

    pub fn clear_completed(&self) -> Result<usize, TaskError> {
        let tasks = self.list()?;
        let count = tasks.len();
        let retained = tasks
            .into_iter()
            .filter(|task| matches!(task.status.as_str(), "pending" | "running"))
            .collect::<Vec<_>>();
        let removed = count.saturating_sub(retained.len());
        self.write(&retained)?;
        Ok(removed)
    }

    fn mutate(&self, id: &str, update: impl FnOnce(&mut Task)) -> Result<Option<Task>, TaskError> {
        let mut tasks = self.list()?;
        let Some(task) = tasks.iter_mut().find(|task| task.id == id) else {
            return Ok(None);
        };
        update(task);
        task.updated_at = timestamp();
        let task = task.clone();
        self.write(&tasks)?;
        Ok(Some(task))
    }

    fn write(&self, tasks: &[Task]) -> Result<(), TaskError> {
        if let Some(directory) = self.path.parent() {
            fs::create_dir_all(directory).map_err(|source| TaskError::CreateDirectory {
                path: directory.to_path_buf(),
                source,
            })?;
        }
        let raw = serde_json::to_string_pretty(tasks).map_err(TaskError::Serialize)?;
        fs::write(&self.path, format!("{raw}\n")).map_err(|source| TaskError::Write {
            path: self.path.clone(),
            source,
        })
    }
}

pub fn tasks_path() -> Result<PathBuf, TaskError> {
    dirs::config_dir()
        .map(|directory| directory.join("mint").join("tasks.json"))
        .ok_or(TaskError::ConfigDirectoryUnavailable)
}

fn timestamp() -> String {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
        .to_string()
}

fn default_max_retries() -> u32 {
    1
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_path(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!("mint-task-{name}-{}.json", std::process::id()))
    }

    #[test]
    fn clears_finished_tasks_but_keeps_active_tasks() {
        let path = test_path("clear");
        let store = TaskStore::open(&path);
        store
            .write(&[
                Task {
                    status: "completed".into(),
                    ..store.add("done").unwrap()
                },
                Task {
                    status: "pending".into(),
                    ..store.add("todo").unwrap()
                },
            ])
            .unwrap();
        assert_eq!(store.clear_completed().unwrap(), 1);
        assert_eq!(store.list().unwrap().len(), 1);
        let _ = fs::remove_file(path);
    }
}

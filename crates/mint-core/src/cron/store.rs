use std::{
    fs,
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};

use chrono::Utc;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use super::schedule::next_run_after;

#[derive(Debug, Error)]
pub enum CronError {
    #[error("unable to determine the user config directory")]
    ConfigDirectoryUnavailable,
    #[error("unable to create cron directory {path}: {source}")]
    CreateDirectory {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("unable to read cron jobs file {path}: {source}")]
    Read {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("unable to parse cron jobs file {path}: {source}")]
    Parse {
        path: PathBuf,
        source: serde_json::Error,
    },
    #[error("unable to serialize cron jobs: {0}")]
    Serialize(serde_json::Error),
    #[error("unable to write cron jobs file {path}: {source}")]
    Write {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("invalid cron schedule {0:?}: {1}")]
    InvalidSchedule(String, String),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CronJob {
    pub id: String,
    pub name: String,
    pub schedule: String,
    pub task: String,
    pub workspace: PathBuf,
    pub enabled: bool,
    pub created_at: String,
    pub next_run: String,
    pub last_run_at: Option<String>,
    pub last_status: Option<String>,
    pub last_summary: Option<String>,
}

/// Request body shape for creating a job from the GUI/API layer (Tauri
/// command / `POST /api/cron`), separate from [`CronJob`] since callers don't
/// supply `id`/`enabled`/timestamps themselves.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CronJobDraft {
    pub name: String,
    pub schedule: String,
    pub task: String,
    pub workspace: PathBuf,
}

#[derive(Clone)]
pub struct CronStore {
    path: PathBuf,
}

impl CronStore {
    pub fn open_default() -> Result<Self, CronError> {
        Ok(Self::open(cron_jobs_path()?))
    }

    pub fn open(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }

    pub fn list(&self) -> Result<Vec<CronJob>, CronError> {
        if !self.path.exists() {
            return Ok(Vec::new());
        }
        let raw = fs::read_to_string(&self.path).map_err(|source| CronError::Read {
            path: self.path.clone(),
            source,
        })?;
        serde_json::from_str(&raw).map_err(|source| CronError::Parse {
            path: self.path.clone(),
            source,
        })
    }

    pub fn get(&self, id: &str) -> Result<Option<CronJob>, CronError> {
        Ok(self.list()?.into_iter().find(|job| job.id == id))
    }

    pub fn add(
        &self,
        name: impl Into<String>,
        schedule: impl Into<String>,
        task: impl Into<String>,
        workspace: PathBuf,
    ) -> Result<CronJob, CronError> {
        let schedule = schedule.into();
        let next_run = next_run_after(&schedule, Utc::now())
            .map_err(|message| CronError::InvalidSchedule(schedule.clone(), message))?;
        let now = timestamp();
        let job = CronJob {
            id: now.clone(),
            name: name.into(),
            schedule,
            task: task.into(),
            workspace,
            enabled: true,
            created_at: now,
            next_run: next_run.to_rfc3339(),
            last_run_at: None,
            last_status: None,
            last_summary: None,
        };
        let mut jobs = self.list()?;
        jobs.push(job.clone());
        self.write(&jobs)?;

        // Best-effort: give the task's conversation its name up front so it
        // shows up in the sidebar immediately, titled, instead of waiting
        // for the generic "New chat" -> first-message rename to happen on
        // its first run. Not fatal if the memory store is unavailable — the
        // scheduled task itself is still created either way.
        if let Ok(memory) = crate::memory::MemoryStore::open_default() {
            let _ = memory.ensure_named_chat_session(
                &super::scheduler::chat_id_for_job(&job.id),
                &job.name,
                "conversation",
            );
        }

        Ok(job)
    }

    pub fn remove(&self, id: &str) -> Result<bool, CronError> {
        let mut jobs = self.list()?;
        let before = jobs.len();
        jobs.retain(|job| job.id != id);
        let removed = jobs.len() != before;
        if removed {
            self.write(&jobs)?;
        }
        Ok(removed)
    }

    pub fn set_enabled(&self, id: &str, enabled: bool) -> Result<Option<CronJob>, CronError> {
        self.mutate(id, |job| job.enabled = enabled)
    }

    /// Moves `next_run` forward to the next occurrence after now, *before*
    /// the job actually runs — so a job whose execution outlasts the
    /// scheduler's tick interval can't be picked up and re-fired a second
    /// time on the following tick.
    pub fn advance_next_run(&self, id: &str) -> Result<Option<CronJob>, CronError> {
        let mut jobs = self.list()?;
        let Some(job) = jobs.iter_mut().find(|job| job.id == id) else {
            return Ok(None);
        };
        let next = next_run_after(&job.schedule, Utc::now())
            .map_err(|message| CronError::InvalidSchedule(job.schedule.clone(), message))?;
        job.next_run = next.to_rfc3339();
        let updated = job.clone();
        self.write(&jobs)?;
        Ok(Some(updated))
    }

    pub fn record_run(
        &self,
        id: &str,
        status: &str,
        summary: Option<String>,
    ) -> Result<Option<CronJob>, CronError> {
        self.mutate(id, |job| {
            job.last_run_at = Some(Utc::now().to_rfc3339());
            job.last_status = Some(status.to_string());
            if summary.is_some() {
                job.last_summary = summary.clone();
            }
        })
    }

    fn mutate(
        &self,
        id: &str,
        f: impl FnOnce(&mut CronJob),
    ) -> Result<Option<CronJob>, CronError> {
        let mut jobs = self.list()?;
        let Some(job) = jobs.iter_mut().find(|job| job.id == id) else {
            return Ok(None);
        };
        f(job);
        let updated = job.clone();
        self.write(&jobs)?;
        Ok(Some(updated))
    }

    fn write(&self, jobs: &[CronJob]) -> Result<(), CronError> {
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent).map_err(|source| CronError::CreateDirectory {
                path: parent.to_path_buf(),
                source,
            })?;
        }
        let raw = serde_json::to_string_pretty(jobs).map_err(CronError::Serialize)?;
        fs::write(&self.path, raw).map_err(|source| CronError::Write {
            path: self.path.clone(),
            source,
        })
    }
}

pub fn cron_jobs_path() -> Result<PathBuf, CronError> {
    dirs::config_dir()
        .map(|directory| directory.join("mint").join("cron-jobs.json"))
        .ok_or(CronError::ConfigDirectoryUnavailable)
}

fn timestamp() -> String {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_store() -> CronStore {
        // `timestamp()` alone collides across tests running in parallel
        // threads of the same process within the same millisecond; a random
        // suffix guarantees each test gets its own file.
        CronStore::open(std::env::temp_dir().join(format!(
            "mint-cron-test-{}-{}.json",
            std::process::id(),
            uuid::Uuid::new_v4()
        )))
    }

    #[test]
    fn add_list_get_round_trip() {
        let store = temp_store();
        let job = store
            .add("stock report", "0 8 * * *", "fetch stock prices", PathBuf::from("/tmp"))
            .unwrap();
        assert!(job.enabled);
        assert!(job.last_run_at.is_none());

        let listed = store.list().unwrap();
        assert_eq!(listed.len(), 1);
        assert_eq!(store.get(&job.id).unwrap().unwrap().id, job.id);
    }

    #[test]
    fn add_rejects_invalid_schedule() {
        let store = temp_store();
        assert!(store
            .add("bad", "not a cron expression", "task", PathBuf::from("/tmp"))
            .is_err());
    }

    #[test]
    fn set_enabled_and_remove() {
        let store = temp_store();
        let job = store
            .add("job", "0 8 * * *", "task", PathBuf::from("/tmp"))
            .unwrap();

        let disabled = store.set_enabled(&job.id, false).unwrap().unwrap();
        assert!(!disabled.enabled);

        assert!(store.remove(&job.id).unwrap());
        assert!(store.get(&job.id).unwrap().is_none());
        assert!(!store.remove(&job.id).unwrap());
    }

    #[test]
    fn record_run_updates_status_and_summary() {
        let store = temp_store();
        let job = store
            .add("job", "0 8 * * *", "task", PathBuf::from("/tmp"))
            .unwrap();

        let updated = store
            .record_run(&job.id, "success", Some("done".to_string()))
            .unwrap()
            .unwrap();
        assert_eq!(updated.last_status.as_deref(), Some("success"));
        assert_eq!(updated.last_summary.as_deref(), Some("done"));
        assert!(updated.last_run_at.is_some());
    }
}

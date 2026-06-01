use std::path::PathBuf;

use mint_core::TaskStore;

fn test_path(name: &str) -> PathBuf {
    std::env::temp_dir().join(format!(
        "mint-task-integration-{name}-{}.json",
        std::process::id()
    ))
}

#[test]
fn adds_and_reads_task() {
    let path = test_path("add");
    let _ = std::fs::remove_file(&path);
    let store = TaskStore::open(&path);
    let task = store.add("migrate backend").unwrap();
    assert_eq!(
        store.get(&task.id).unwrap().unwrap().description,
        "migrate backend"
    );
    let _ = std::fs::remove_file(path);
}

#[test]
fn retries_failed_tasks_once_before_marking_them_failed() {
    let path = test_path("retry");
    let _ = std::fs::remove_file(&path);
    let store = TaskStore::open(&path);
    let task = store.add("retry task").unwrap();
    assert_eq!(
        store
            .fail_with_retry(&task.id, "first")
            .unwrap()
            .unwrap()
            .status,
        "pending"
    );
    assert_eq!(
        store
            .fail_with_retry(&task.id, "second")
            .unwrap()
            .unwrap()
            .status,
        "failed"
    );
    let _ = std::fs::remove_file(path);
}

#[test]
fn resumes_interrupted_running_tasks() {
    let path = test_path("resume");
    let _ = std::fs::remove_file(&path);
    let store = TaskStore::open(&path);
    let task = store.add("resume task").unwrap();
    store.update_status(&task.id, "running", None).unwrap();
    assert_eq!(store.resume_running().unwrap()[0].status, "pending");
    let _ = std::fs::remove_file(path);
}

#[test]
fn records_headless_task_lifecycle() {
    let path = test_path("lifecycle");
    let _ = std::fs::remove_file(&path);
    let store = TaskStore::open(&path);
    let task = store.add("background audit").unwrap();
    store.update_status(&task.id, "running", None).unwrap();
    store
        .add_checkpoint(&task.id, serde_json::json!({ "phase": "started" }))
        .unwrap();
    store
        .add_artifact(&task.id, serde_json::json!({ "type": "proposal" }))
        .unwrap();
    let completed = store
        .update_status(
            &task.id,
            "completed",
            Some(serde_json::json!({ "summary": "done" })),
        )
        .unwrap()
        .unwrap();
    assert_eq!(completed.status, "completed");
    assert_eq!(completed.checkpoints.len(), 1);
    assert_eq!(completed.artifacts.len(), 1);
    assert_eq!(completed.result.unwrap()["summary"], "done");
    let _ = std::fs::remove_file(path);
}

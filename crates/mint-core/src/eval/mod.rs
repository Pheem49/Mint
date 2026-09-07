use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BenchmarkTask {
    pub id: String,
    pub title: String,
    pub prompt: String,
    #[serde(default)]
    pub setup_commands: Vec<String>,
    #[serde(default)]
    pub verify_commands: Vec<String>,
    pub max_files_modified: Option<usize>,
    #[serde(default)]
    pub expected_modified_files: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskEvalResult {
    pub task_id: String,
    pub title: String,
    pub passed: bool,
    pub test_passed: bool,
    pub typecheck_passed: bool,
    pub files_modified_count: usize,
    pub files_constraint_met: bool,
    pub total_tokens: usize,
    pub tool_calls_count: usize,
    pub duration_secs: f64,
    pub retries_count: usize,
    pub error_message: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BenchmarkSuite {
    pub name: String,
    pub version: String,
    pub tasks: Vec<BenchmarkTask>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BenchmarkReport {
    pub suite_name: String,
    pub total_tasks: usize,
    pub passed_tasks: usize,
    pub success_rate: f64,
    pub avg_duration_secs: f64,
    pub avg_tokens: usize,
    pub avg_tool_calls: f64,
    pub avg_retries: f64,
    pub task_results: Vec<TaskEvalResult>,
}

pub fn load_suite_from_file(path: &Path) -> Result<BenchmarkSuite, String> {
    let raw = std::fs::read_to_string(path)
        .map_err(|e| format!("Failed to read benchmark suite file {}: {e}", path.display()))?;
    load_suite_from_json(&raw)
}

pub fn load_suite_from_json(raw: &str) -> Result<BenchmarkSuite, String> {
    serde_json::from_str(raw).map_err(|e| format!("Invalid benchmark suite JSON: {e}"))
}

pub fn evaluate_task_result(
    task: &BenchmarkTask,
    total_tokens: usize,
    tool_calls_count: usize,
    files_modified: &[String],
    duration_secs: f64,
    retries_count: usize,
    test_passed: bool,
    typecheck_passed: bool,
    error_message: Option<String>,
) -> TaskEvalResult {
    let files_modified_count = files_modified.len();
    let files_constraint_met = match task.max_files_modified {
        Some(max) => files_modified_count <= max,
        None => true,
    };

    let passed = test_passed && typecheck_passed && files_constraint_met && error_message.is_none();

    TaskEvalResult {
        task_id: task.id.clone(),
        title: task.title.clone(),
        passed,
        test_passed,
        typecheck_passed,
        files_modified_count,
        files_constraint_met,
        total_tokens,
        tool_calls_count,
        duration_secs,
        retries_count,
        error_message,
    }
}

pub fn aggregate_benchmark_report(
    suite_name: &str,
    results: Vec<TaskEvalResult>,
) -> BenchmarkReport {
    let total_tasks = results.len();
    let passed_tasks = results.iter().filter(|r| r.passed).count();
    let success_rate = if total_tasks > 0 {
        (passed_tasks as f64 / total_tasks as f64) * 100.0
    } else {
        0.0
    };

    let total_duration: f64 = results.iter().map(|r| r.duration_secs).sum();
    let avg_duration_secs = if total_tasks > 0 {
        total_duration / total_tasks as f64
    } else {
        0.0
    };

    let total_tokens: usize = results.iter().map(|r| r.total_tokens).sum();
    let avg_tokens = if total_tasks > 0 {
        total_tokens / total_tasks
    } else {
        0
    };

    let total_tools: usize = results.iter().map(|r| r.tool_calls_count).sum();
    let avg_tool_calls = if total_tasks > 0 {
        total_tools as f64 / total_tasks as f64
    } else {
        0.0
    };

    let total_retries: usize = results.iter().map(|r| r.retries_count).sum();
    let avg_retries = if total_tasks > 0 {
        total_retries as f64 / total_tasks as f64
    } else {
        0.0
    };

    BenchmarkReport {
        suite_name: suite_name.to_string(),
        total_tasks,
        passed_tasks,
        success_rate,
        avg_duration_secs,
        avg_tokens,
        avg_tool_calls,
        avg_retries,
        task_results: results,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_benchmark_eval_and_reporting() {
        let task = BenchmarkTask {
            id: "task-001".into(),
            title: "Fix Auth Bug".into(),
            prompt: "Fix token validation in AuthService".into(),
            setup_commands: vec![],
            verify_commands: vec!["cargo test".into()],
            max_files_modified: Some(3),
            expected_modified_files: vec!["src/auth.rs".into()],
        };

        let result = evaluate_task_result(
            &task,
            12_500,
            8,
            &["src/auth.rs".into(), "tests/auth_test.rs".into()],
            45.2,
            1,
            true,
            true,
            None,
        );

        assert!(result.passed);
        assert!(result.files_constraint_met);

        let report = aggregate_benchmark_report("SWE-Mint-Mini", vec![result]);
        assert_eq!(report.total_tasks, 1);
        assert_eq!(report.passed_tasks, 1);
        assert_eq!(report.success_rate, 100.0);
        assert_eq!(report.avg_tokens, 12_500);
    }
}

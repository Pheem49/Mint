use std::collections::BTreeMap;

use serde_json::Value;
use sha2::{Digest, Sha256};

use super::{OrchestrationError, ToolResultEntry, shell_result_failed};

const WARN_AFTER: usize = 3;
const STOP_AFTER: usize = 5;

#[derive(Default)]
pub(super) struct RepeatedFailures {
    tools: BTreeMap<String, Failure>,
}

struct Failure {
    fingerprint: [u8; 32],
    rounds: usize,
    last_step: usize,
}

impl RepeatedFailures {
    /// Compare tool, complete arguments and the failure, excluding Mint's own
    /// retry counters and tips. Keep only digests, never another copy of code.
    /// Other tools' successes cannot hide a repeated failure of this tool.
    pub(super) fn observe(
        &mut self,
        step: usize,
        results: &mut [ToolResultEntry],
    ) -> Result<Vec<String>, OrchestrationError> {
        let mut warnings = Vec::new();
        for (_, tool, input, result) in results {
            let error = result.split("\n\n[System Tip:").next().unwrap_or(result);
            let error = error
                .split(". No tool was executed for this call.")
                .next()
                .unwrap_or(error);
            let failed = error.starts_with("Error")
                || error.starts_with("Blocked")
                || error.starts_with("Skipped")
                || (matches!(
                    tool.as_str(),
                    "run_shell" | "verify" | "run_tests" | "run_typecheck" | "run_linter"
                ) && shell_result_failed(error));
            if !failed {
                // A successful invocation of the same tool breaks its streak.
                // A denied/intercepted edit is not evidence of task progress.
                if !error.starts_with("User denied") && !error.starts_with("User did not approve") {
                    self.tools.remove(tool);
                }
                continue;
            }
            let fingerprint = fingerprint(tool, input, error);
            let state = self.tools.entry(tool.clone()).or_insert(Failure {
                fingerprint,
                rounds: 0,
                last_step: 0,
            });
            if state.last_step == step {
                continue; // The model has not seen this batch's feedback yet.
            }
            if state.fingerprint != fingerprint {
                state.fingerprint = fingerprint;
                state.rounds = 0;
            }
            state.last_step = step;
            state.rounds += 1;
            if state.rounds >= STOP_AFTER {
                return Err(OrchestrationError::Agent(format!(
                    "Stopped a repeated failure loop: tool '{tool}' returned the same error with the same arguments in {STOP_AFTER} rounds, despite a warning. Task incomplete. Change the approach or resolve the tool failure before continuing."
                )));
            }
            if state.rounds >= WARN_AFTER {
                let warning = format!(
                    "Loop warning: tool '{tool}' has failed with the same arguments and error in {} rounds. Change the arguments or choose another approach. Repeating this unchanged failure for {STOP_AFTER} rounds will stop the task. Do not claim the task is complete.",
                    state.rounds
                );
                result.push_str(&format!("\n\n[System Tip: {warning}]"));
                warnings.push(warning);
            }
        }
        Ok(warnings)
    }
}

fn fingerprint(tool: &str, input: &Value, error: &str) -> [u8; 32] {
    // Canonical object ordering keeps equivalent JSON arguments equivalent,
    // including builds where serde_json's preserve_order feature is enabled.
    fn canonical(value: &Value) -> String {
        match value {
            Value::Object(object) => {
                let ordered: BTreeMap<_, _> = object.iter().collect();
                format!(
                    "{{{}}}",
                    ordered
                        .into_iter()
                        .map(|(key, value)| {
                            format!(
                                "{}:{}",
                                serde_json::to_string(key).unwrap(),
                                canonical(value)
                            )
                        })
                        .collect::<Vec<_>>()
                        .join(",")
                )
            }
            Value::Array(values) => format!(
                "[{}]",
                values.iter().map(canonical).collect::<Vec<_>>().join(",")
            ),
            _ => value.to_string(),
        }
    }
    let payload = serde_json::json!([tool, canonical(input), error.trim()]).to_string();
    Sha256::digest(payload.as_bytes()).into()
}

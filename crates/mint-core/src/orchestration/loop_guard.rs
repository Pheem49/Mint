//! Repetition & Stuck-Loop Guard Harness
//!
//! Tracks action fingerprints across the agent loop to detect consecutive identical
//! invocations and oscillatory cycles (A -> B -> A -> B). When a stuck loop is detected,
//! it intervenes by intercepting redundant execution and returning structured self-correction
//! guidance to the model without crashing or aborting the turn.

use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, VecDeque};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActionFingerprint {
    pub action: String,
    pub input_hash: [u8; 32],
    pub summary_preview: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LoopGuardDecision {
    /// Safe to execute
    Allow,
    /// Safe to execute, but append a soft warning notice to observation
    WarnConsecutiveRepeat {
        count: usize,
        action: String,
        notice: String,
    },
    /// Intercept execution to prevent token burn; return guidance as observation
    InterceptStuckLoop {
        action: String,
        reason: String,
        instruction: String,
    },
}

#[derive(Debug, Clone)]
pub struct RepetitionGuard {
    history: VecDeque<ActionFingerprint>,
    last_fingerprint: Option<ActionFingerprint>,
    consecutive_streak: usize,
    max_history: usize,
    warn_threshold: usize,
    intercept_threshold: usize,
}

impl Default for RepetitionGuard {
    fn default() -> Self {
        Self {
            history: VecDeque::with_capacity(16),
            last_fingerprint: None,
            consecutive_streak: 0,
            max_history: 16,
            warn_threshold: 2,
            intercept_threshold: 3,
        }
    }
}

impl RepetitionGuard {
    pub fn new(warn_threshold: usize, intercept_threshold: usize) -> Self {
        Self {
            history: VecDeque::with_capacity(16),
            last_fingerprint: None,
            consecutive_streak: 0,
            max_history: 16,
            warn_threshold,
            intercept_threshold,
        }
    }

    /// Evaluates whether an action should be allowed, warned, or intercepted.
    pub fn check(&mut self, action: &str, input: &Value) -> LoopGuardDecision {
        if action == "finish" {
            return LoopGuardDecision::Allow;
        }

        let fp = compute_action_fingerprint(action, input);

        // 1. Check consecutive identical invocation
        if let Some(ref last) = self.last_fingerprint {
            if last.action == fp.action && last.input_hash == fp.input_hash {
                self.consecutive_streak += 1;
                let count = self.consecutive_streak;

                if count >= self.intercept_threshold {
                    return LoopGuardDecision::InterceptStuckLoop {
                        action: action.to_string(),
                        reason: format!(
                            "Identical action '{action}' ({}) was called {count} consecutive times without intervening state changes.",
                            fp.summary_preview
                        ),
                        instruction: format!(
                            "[Stuck-Loop Guard: Intercepted repeated call to '{action}']\n\
                            You have already run this exact action with identical parameters {count} times consecutively. \
                            Mint has intercepted this call to protect your context budget and prevent runaway loops.\n\
                            NEXT STEPS:\n\
                            1. Analyze why previous attempts did not advance the task.\n\
                            2. If an error occurred, inspect the error output or read related source files.\n\
                            3. Apply a concrete modification (e.g. apply_patch / write_file) or conclude with 'finish' explaining the blocker."
                        ),
                    };
                } else if count >= self.warn_threshold {
                    self.record_history(fp);
                    return LoopGuardDecision::WarnConsecutiveRepeat {
                        count,
                        action: action.to_string(),
                        notice: format!(
                            "[Loop Notice: You executed this identical '{action}' on the previous step with the same parameters. \
                            If you did not modify files, re-running it may produce the same output. Consider a different approach or verify code changes.]"
                        ),
                    };
                }
            } else {
                self.consecutive_streak = 1;
                self.last_fingerprint = Some(fp.clone());
            }
        } else {
            self.consecutive_streak = 1;
            self.last_fingerprint = Some(fp.clone());
        }

        // 2. Check cyclical oscillation (A -> B -> A -> B)
        if self.history.len() >= 3 {
            let n = self.history.len();
            if self.history[n - 2] == fp && self.history[n - 1] == self.history[n - 3] {
                let other = &self.history[n - 1];
                let notice = format!(
                    "[Loop Warning: Oscillating cycle detected between '{}' and '{}'. \
                    Break this cycle by choosing a new action, editing files, or concluding.]",
                    fp.summary_preview, other.summary_preview
                );
                self.record_history(fp);
                return LoopGuardDecision::WarnConsecutiveRepeat {
                    count: 2,
                    action: action.to_string(),
                    notice,
                };
            }
        }

        self.record_history(fp);
        LoopGuardDecision::Allow
    }

    fn record_history(&mut self, fp: ActionFingerprint) {
        if self.history.len() >= self.max_history {
            self.history.pop_front();
        }
        self.history.push_back(fp);
    }

    /// Resets repetition streaks when workspace state changes (file edit, patch applied).
    pub fn notify_state_change(&mut self) {
        self.last_fingerprint = None;
        self.consecutive_streak = 0;
        self.history.clear();
    }
}

pub fn compute_action_fingerprint(action: &str, input: &Value) -> ActionFingerprint {
    let mut hasher = Sha256::new();
    hasher.update(action.as_bytes());
    hasher.update(b"::");

    fn canonical_json_bytes(val: &Value, hasher: &mut Sha256) {
        match val {
            Value::Null => hasher.update(b"null"),
            Value::Bool(b) => {
                if *b {
                    hasher.update(b"true");
                } else {
                    hasher.update(b"false");
                }
            }
            Value::Number(n) => hasher.update(n.to_string().as_bytes()),
            Value::String(s) => {
                hasher.update(b"\"");
                hasher.update(s.as_bytes());
                hasher.update(b"\"");
            }
            Value::Array(arr) => {
                hasher.update(b"[");
                for (i, elem) in arr.iter().enumerate() {
                    if i > 0 {
                        hasher.update(b",");
                    }
                    canonical_json_bytes(elem, hasher);
                }
                hasher.update(b"]");
            }
            Value::Object(map) => {
                let sorted: BTreeMap<_, _> = map.iter().collect();
                hasher.update(b"{");
                for (i, (k, v)) in sorted.iter().enumerate() {
                    if i > 0 {
                        hasher.update(b",");
                    }
                    hasher.update(k.as_bytes());
                    hasher.update(b":");
                    canonical_json_bytes(v, hasher);
                }
                hasher.update(b"}");
            }
        }
    }

    canonical_json_bytes(input, &mut hasher);
    let mut hash = [0u8; 32];
    hash.copy_from_slice(&hasher.finalize());

    let summary_preview = match input {
        Value::Object(map) => {
            if let Some(cmd) = map.get("command").and_then(|v| v.as_str()) {
                format!("command: {cmd}")
            } else if let Some(path) = map.get("path").and_then(|v| v.as_str()) {
                format!("path: {path}")
            } else if let Some(query) = map.get("query").and_then(|v| v.as_str()) {
                format!("query: {query}")
            } else {
                action.to_string()
            }
        }
        _ => action.to_string(),
    };

    ActionFingerprint {
        action: action.to_string(),
        input_hash: hash,
        summary_preview,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn allows_distinct_tool_actions() {
        let mut guard = RepetitionGuard::default();
        assert_eq!(
            guard.check("read_file", &json!({"path": "src/a.rs"})),
            LoopGuardDecision::Allow
        );
        assert_eq!(
            guard.check("read_file", &json!({"path": "src/b.rs"})),
            LoopGuardDecision::Allow
        );
        assert_eq!(
            guard.check("run_shell", &json!({"command": "cargo test"})),
            LoopGuardDecision::Allow
        );
    }

    #[test]
    fn warns_at_two_and_intercepts_at_three_consecutive_calls() {
        let mut guard = RepetitionGuard::default();
        let input = json!({"path": "src/main.rs"});

        // 1st call -> Allow
        assert_eq!(guard.check("read_file", &input), LoopGuardDecision::Allow);

        // 2nd call -> WarnConsecutiveRepeat
        let decision2 = guard.check("read_file", &input);
        match decision2 {
            LoopGuardDecision::WarnConsecutiveRepeat {
                count,
                action,
                notice,
            } => {
                assert_eq!(count, 2);
                assert_eq!(action, "read_file");
                assert!(notice.contains("Loop Notice"));
            }
            other => panic!("Expected WarnConsecutiveRepeat, got {:?}", other),
        }

        // 3rd call -> InterceptStuckLoop
        let decision3 = guard.check("read_file", &input);
        match decision3 {
            LoopGuardDecision::InterceptStuckLoop {
                action,
                instruction,
                ..
            } => {
                assert_eq!(action, "read_file");
                assert!(instruction.contains("Stuck-Loop Guard: Intercepted"));
            }
            other => panic!("Expected InterceptStuckLoop, got {:?}", other),
        }
    }

    #[test]
    fn state_change_resets_repetition_streak() {
        let mut guard = RepetitionGuard::default();
        let input = json!({"command": "cargo test"});

        assert_eq!(guard.check("run_shell", &input), LoopGuardDecision::Allow);
        assert!(matches!(
            guard.check("run_shell", &input),
            LoopGuardDecision::WarnConsecutiveRepeat { .. }
        ));

        // State changes (file edit / patch applied)
        guard.notify_state_change();

        // After state change, running test again is allowed!
        assert_eq!(guard.check("run_shell", &input), LoopGuardDecision::Allow);
    }

    #[test]
    fn detects_oscillating_cycle() {
        let mut guard = RepetitionGuard::default();
        let action_a = "read_file";
        let input_a = json!({"path": "src/a.rs"});

        let action_b = "read_file";
        let input_b = json!({"path": "src/b.rs"});

        // A -> B -> A -> B
        assert_eq!(guard.check(action_a, &input_a), LoopGuardDecision::Allow);
        assert_eq!(guard.check(action_b, &input_b), LoopGuardDecision::Allow);
        assert_eq!(guard.check(action_a, &input_a), LoopGuardDecision::Allow);

        let decision = guard.check(action_b, &input_b);
        match decision {
            LoopGuardDecision::WarnConsecutiveRepeat { notice, .. } => {
                assert!(notice.contains("Oscillating cycle detected"));
            }
            other => panic!("Expected oscillation warning, got {:?}", other),
        }
    }
}

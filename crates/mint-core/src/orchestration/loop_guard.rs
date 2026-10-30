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
    /// Hashes of the most recent read-only tool results (see `observe_result`).
    result_window: VecDeque<[u8; 32]>,
    /// Armed when one outcome repeated `intercept_threshold` times; consumed by the next `check`.
    pending_result_intercept: bool,
}

/// How many recent read-only results are compared for "nothing new" loops.
const RESULT_WINDOW: usize = 8;

impl Default for RepetitionGuard {
    fn default() -> Self {
        Self {
            history: VecDeque::with_capacity(16),
            last_fingerprint: None,
            consecutive_streak: 0,
            max_history: 16,
            warn_threshold: 2,
            intercept_threshold: 3,
            result_window: VecDeque::with_capacity(RESULT_WINDOW),
            pending_result_intercept: false,
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
            result_window: VecDeque::with_capacity(RESULT_WINDOW),
            pending_result_intercept: false,
        }
    }

    /// Evaluates whether an action should be allowed, warned, or intercepted.
    pub fn check(&mut self, action: &str, input: &Value) -> LoopGuardDecision {
        if action == "finish" {
            return LoopGuardDecision::Allow;
        }

        if self.pending_result_intercept {
            self.pending_result_intercept = false;
            self.result_window.clear();
            return LoopGuardDecision::InterceptStuckLoop {
                action: action.to_string(),
                reason: "Recent read-only calls kept returning the same result.".to_string(),
                instruction: format!(
                    "[Stuck-Loop Guard: Intercepted '{action}']\n\
                     The last several observations (screenshots, page text, images) were identical even \
                     though the inputs changed, so another call would not teach you anything new.\n\
                     NEXT STEPS:\n\
                     1. Answer from the information you already have, or\n\
                     2. Take a genuinely different approach (different URL, a text/DOM read instead of an image), or\n\
                     3. Call 'finish' and explain the blocker."
                ),
            };
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
        self.result_window.clear();
        self.pending_result_intercept = false;
    }

    /// Records the observable result of a read-only tool call (text plus any
    /// attached image bytes, hashed by the caller into `result`). Catches loops
    /// whose *inputs* differ each time (new screenshot filename, new scroll
    /// offset) but whose *outcome* never changes. Returns a notice to append to
    /// the tool result when the same outcome repeats; the third repeat also arms
    /// an intercept of the next non-`finish` call.
    pub fn observe_result(&mut self, action: &str, result: &str) -> Option<String> {
        let mut hasher = Sha256::new();
        hasher.update(result.trim().as_bytes());
        let mut hash = [0u8; 32];
        hash.copy_from_slice(&hasher.finalize());

        if self.result_window.len() >= RESULT_WINDOW {
            self.result_window.pop_front();
        }
        self.result_window.push_back(hash);
        let count = self.result_window.iter().filter(|h| **h == hash).count();

        if count >= self.intercept_threshold {
            self.pending_result_intercept = true;
        }
        if count >= self.warn_threshold {
            Some(format!(
                "[Loop Notice: '{action}' returned the same result {count} times in recent steps \
                 even though its inputs changed. Nothing new is being learned. Use the information \
                 you already have, change approach, or call finish.]"
            ))
        } else {
            None
        }
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
    fn identical_results_across_different_inputs_trigger_intercept() {
        // screenshot -> view_image -> scroll cycle: every input differs but the
        // observed result (same image) never changes.
        let mut guard = RepetitionGuard::default();
        let calls = [
            ("run_shell", json!({"command": "chrome --screenshot=/tmp/a.png u"})),
            ("view_image", json!({"path": "/tmp/a.png"})),
            ("run_shell", json!({"command": "chrome --screenshot=/tmp/b.png u"})),
        ];
        let mut notices = Vec::new();
        for (action, input) in &calls {
            assert_eq!(guard.check(action, input), LoopGuardDecision::Allow);
            notices.push(guard.observe_result(action, "same-page-bytes"));
        }
        assert!(notices[0].is_none());
        assert!(notices[1].as_deref().is_some_and(|n| n.contains("Loop Notice")));
        assert!(notices[2].is_some());

        // The next non-finish call is intercepted even though its input is new.
        assert!(matches!(
            guard.check("view_image", &json!({"path": "/tmp/c.png"})),
            LoopGuardDecision::InterceptStuckLoop { .. }
        ));
        // `finish` is never blocked.
        assert_eq!(
            guard.check("finish", &json!({})),
            LoopGuardDecision::Allow
        );
    }

    #[test]
    fn distinct_results_never_trigger_result_guard() {
        let mut guard = RepetitionGuard::default();
        for i in 0..10 {
            let input = json!({"path": format!("f{i}")});
            assert_eq!(guard.check("read_file", &input), LoopGuardDecision::Allow);
            assert!(guard.observe_result("read_file", &format!("body {i}")).is_none());
        }
    }

    #[test]
    fn state_change_clears_result_window_and_pending_intercept() {
        let mut guard = RepetitionGuard::default();
        for _ in 0..3 {
            guard.observe_result("view_image", "same");
        }
        guard.notify_state_change();
        assert_eq!(
            guard.check("view_image", &json!({"path": "x"})),
            LoopGuardDecision::Allow
        );
        assert!(guard.observe_result("view_image", "same").is_none());
    }

    #[test]
    fn intercept_fires_once_then_guard_resets() {
        let mut guard = RepetitionGuard::default();
        for _ in 0..3 {
            guard.observe_result("view_image", "same");
        }
        assert!(matches!(
            guard.check("scroll", &json!({"y": 1})),
            LoopGuardDecision::InterceptStuckLoop { .. }
        ));
        assert_eq!(
            guard.check("read_file", &json!({"path": "a"})),
            LoopGuardDecision::Allow
        );
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

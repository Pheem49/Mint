//! In-memory liveness tracking for the messaging bridges in `channels.rs`.
//!
//! `channels::restarting_loop` catches crashes/panics and retries them
//! automatically, but that alone only answers "did it ever crash" — not "is
//! it alive right now." This module tracks a per-bridge last-success
//! timestamp (recorded by each bridge loop at its own natural per-cycle
//! success point — a poll response, a heartbeat, ...) alongside crash
//! history, and `api_server` surfaces a snapshot of it over HTTP so the
//! operator can check bridge health remotely (WebUI over an SSH tunnel/
//! Tailscale) instead of having to SSH in and read journald.

use std::collections::HashMap;
use std::sync::{LazyLock, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BridgeHealth {
    pub name: &'static str,
    pub started_at: Option<u64>,
    pub last_success_at: Option<u64>,
    pub last_error_at: Option<u64>,
    pub last_error: Option<String>,
    pub consecutive_failures: u32,
}

impl BridgeHealth {
    fn new(name: &'static str) -> Self {
        Self {
            name,
            started_at: None,
            last_success_at: None,
            last_error_at: None,
            last_error: None,
            consecutive_failures: 0,
        }
    }
}

static REGISTRY: LazyLock<Mutex<HashMap<&'static str, BridgeHealth>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

fn now_unix() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0)
}

fn with_entry(name: &'static str, update: impl FnOnce(&mut BridgeHealth)) {
    let mut registry = REGISTRY.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    let entry = registry
        .entry(name)
        .or_insert_with(|| BridgeHealth::new(name));
    update(entry);
}

/// Marks a bridge as having just started its restart-supervised loop.
pub fn record_started(name: &'static str) {
    with_entry(name, |entry| entry.started_at = Some(now_unix()));
}

/// Marks a bridge as having just completed one poll/connect cycle
/// successfully — call this from inside each bridge loop, not from
/// `restarting_loop` (which only observes whole-loop crashes/restarts, not
/// per-cycle liveness).
pub fn record_success(name: &'static str) {
    with_entry(name, |entry| {
        entry.last_success_at = Some(now_unix());
        entry.consecutive_failures = 0;
    });
}

/// Marks a bridge's whole loop as having crashed (error or panic) and about
/// to retry after backoff.
pub fn record_error(name: &'static str, message: &str) {
    with_entry(name, |entry| {
        entry.last_error_at = Some(now_unix());
        entry.last_error = Some(message.to_string());
        entry.consecutive_failures += 1;
    });
}

/// A sorted-by-name snapshot of every bridge that has run at least once this
/// process — a bridge that's disabled and has never started simply won't
/// appear (callers pair this with `config.extra` to show disabled bridges
/// too, same as `channels`'s own `enableXBridge` flags).
pub fn snapshot() -> Vec<BridgeHealth> {
    let registry = REGISTRY.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    let mut items: Vec<BridgeHealth> = registry.values().cloned().collect();
    items.sort_by(|a, b| a.name.cmp(b.name));
    items
}

#[cfg(test)]
mod tests {
    use super::*;

    // Each bridge name is unique per test to avoid cross-test interference
    // on the shared process-global `REGISTRY`.

    #[test]
    fn a_fresh_bridge_has_no_history() {
        let snapshot = snapshot();
        assert!(!snapshot.iter().any(|b| b.name == "test-fresh-bridge"));
    }

    #[test]
    fn success_clears_the_failure_streak() {
        record_started("test-success-bridge");
        record_error("test-success-bridge", "boom");
        record_error("test-success-bridge", "boom again");
        record_success("test-success-bridge");

        let entry = snapshot()
            .into_iter()
            .find(|b| b.name == "test-success-bridge")
            .expect("bridge should be present after record_started");
        assert_eq!(entry.consecutive_failures, 0);
        assert!(entry.last_success_at.is_some());
        assert!(entry.started_at.is_some());
    }

    #[test]
    fn errors_accumulate_a_consecutive_failure_count() {
        record_error("test-error-bridge", "first");
        record_error("test-error-bridge", "second");

        let entry = snapshot()
            .into_iter()
            .find(|b| b.name == "test-error-bridge")
            .expect("bridge should be present after record_error");
        assert_eq!(entry.consecutive_failures, 2);
        assert_eq!(entry.last_error.as_deref(), Some("second"));
    }
}

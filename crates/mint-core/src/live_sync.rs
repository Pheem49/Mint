//! Detects `interaction_memories` rows written to the "cli" chat by another
//! surface (web/desktop) while an interactive `mint` CLI session is open on
//! the same machine, and queues a short notice for the CLI's prompt loop to
//! print. Mirrors [`crate::push_linked_folder_notice`]/
//! [`crate::take_linked_folder_notices`] (a process-global
//! `LazyLock<Mutex<Vec<String>>>` notice queue) plus
//! [`crate::cron::scheduler`]'s self-healing background-loop shape.
//!
//! This is DB polling rather than a push through the local API server: the
//! interactive CLI already talks to the SQLite DB in-process via
//! `MemoryStore` and has no dependency on `mint web`/`start_api_server`
//! being up. Polling keeps that true — nothing here talks to the API server
//! or the frontend.

use std::sync::LazyLock;
use std::sync::Mutex;
use std::sync::atomic::{AtomicI64, Ordering};
use std::time::Duration;

use crate::agent::memory::{CHAT_CLI_ID, MemoryStore};

/// How often the background loop re-checks `interaction_memories` for the
/// "cli" chat. 1.5s: fast enough that a message sent from web/desktop shows
/// up on an idle/typing terminal well under the ~2s a human notices as
/// "instant", cheap enough that a single small `SELECT ... LIMIT 20` every
/// 1.5s is a non-issue for a local SQLite file that otherwise sees one write
/// per human turn.
const POLL_INTERVAL: Duration = Duration::from_millis(1500);

/// How many most-recent rows to re-check each tick. Bigger than the `1`
/// you'd need for "just the latest" so several messages sent in a burst from
/// another surface between two polls are all surfaced, not just the newest.
const POLL_BATCH: usize = 20;

/// Chars kept from each side of a synced interaction for the notice preview.
const PREVIEW_CHARS: usize = 60;

static LIVE_SYNC_NOTICES: LazyLock<Mutex<Vec<String>>> = LazyLock::new(|| Mutex::new(Vec::new()));

/// Highest `interaction_memories.id` (chat_id = "cli") this process already
/// accounts for — either because it wrote the row itself (see
/// [`note_own_interaction`]) or because a previous poll tick already
/// surfaced it. `-1` until `start_live_sync_poller`'s first tick establishes
/// the real starting point, so nothing already in the DB at CLI startup is
/// (mis)reported as "new".
static LAST_SEEN_ID: AtomicI64 = AtomicI64::new(-1);

/// Drain any live-sync notices for the foreground prompt loop to print —
/// call alongside `take_linked_folder_notices()` /
/// `bg_shell::take_finished_notices()` in the idle-tick branch of
/// `read_line_interactive`.
pub fn take_live_sync_notices() -> Vec<String> {
    std::mem::take(&mut *LIVE_SYNC_NOTICES.lock().unwrap())
}

/// Raise the watermark to at least `id`. Called by `mint-cli` right after it
/// finishes writing its own turn to chat_id="cli", so the next poll tick
/// doesn't mistake the user's own just-sent message for one that arrived
/// from another surface. Also reused internally by the poller itself once
/// it has queued notices for a batch, so both "own write" and "already
/// notified" collapse into one watermark.
pub fn note_own_interaction(id: i64) {
    let mut current = LAST_SEEN_ID.load(Ordering::SeqCst);
    while id > current {
        match LAST_SEEN_ID.compare_exchange_weak(current, id, Ordering::SeqCst, Ordering::SeqCst) {
            Ok(_) => break,
            Err(actual) => current = actual,
        }
    }
}

/// Starts the polling loop. Mirrors
/// [`crate::cron::scheduler::start_cron_scheduler`]'s runtime-detection
/// dance: ride the caller's Tokio runtime if there is one, else spin up a
/// dedicated thread + runtime. Detached, no join handle, no shutdown signal
/// — like the cron scheduler and the messaging-channel bridges, it simply
/// lives for as long as the process does (the interactive CLI exiting kills
/// it for free). Call exactly once, from `run_interactive_chat()` — NOT from
/// `mint gateway` or any other headless path that has no prompt loop to
/// drain notices into.
pub fn start_live_sync_poller() {
    if tokio::runtime::Handle::try_current().is_ok() {
        tokio::spawn(restarting_loop());
    } else {
        std::thread::spawn(|| {
            if let Ok(runtime) = tokio::runtime::Runtime::new() {
                runtime.block_on(restarting_loop());
            }
        });
    }
}

async fn restarting_loop() {
    initialize_watermark().await;
    loop {
        tick().await;
        tokio::time::sleep(POLL_INTERVAL).await;
    }
}

/// Seeds `LAST_SEEN_ID` from whatever is already in the DB at startup, so
/// the CLI's own prior history never gets replayed as a "live" notice.
async fn initialize_watermark() {
    if let Ok(memory) = MemoryStore::open_default()
        && let Ok(latest) = memory.recent_interactions_for_chat(CHAT_CLI_ID, 1)
    {
        let start_id = latest.first().map(|row| row.id).unwrap_or(0);
        LAST_SEEN_ID.store(start_id, Ordering::SeqCst);
    }
    // If either call fails (e.g. DB briefly locked at startup), LAST_SEEN_ID
    // stays at -1 and the first tick's rows all look "new" — a one-time
    // false burst of notices for the CLI's own recent history, not a crash.
    // Acceptable: rare, self-correcting after one tick, and strictly better
    // than silently never polling.
}

async fn tick() {
    let Ok(memory) = MemoryStore::open_default() else {
        return;
    };
    let Ok(rows) = memory.recent_interactions_for_chat(CHAT_CLI_ID, POLL_BATCH) else {
        return;
    };
    let last_seen = LAST_SEEN_ID.load(Ordering::SeqCst);
    let mut new_rows: Vec<_> = rows.into_iter().filter(|row| row.id > last_seen).collect();
    if new_rows.is_empty() {
        return;
    }
    // `recent_interactions_for_chat` is newest-first; print oldest-first.
    new_rows.sort_by_key(|row| row.id);
    let max_id = new_rows.last().map(|row| row.id).unwrap_or(last_seen);

    let mut queue = LIVE_SYNC_NOTICES.lock().unwrap();
    for row in &new_rows {
        queue.push(format!(
            "[synced] {} → {}",
            preview(&row.user_text),
            preview(&row.ai_text)
        ));
    }
    drop(queue);
    note_own_interaction(max_id);
}

fn preview(text: &str) -> String {
    let clean: String = text.split_whitespace().collect::<Vec<_>>().join(" ");
    let truncated: String = clean.chars().take(PREVIEW_CHARS).collect();
    if clean.chars().count() > PREVIEW_CHARS {
        format!("{truncated}…")
    } else {
        truncated
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Mirrors `lib.rs::linked_folder_notice_tests` — exercises only the
    // in-memory queue/watermark bookkeeping, not `tick()`'s DB access.

    #[test]
    fn pushed_notices_are_drained_exactly_once() {
        let _ = take_live_sync_notices(); // isolate from other tests
        LIVE_SYNC_NOTICES
            .lock()
            .unwrap()
            .push("[synced] hi → hello there".to_string());
        let notices = take_live_sync_notices();
        assert_eq!(notices.len(), 1);
        assert!(notices[0].contains("hello there"));
        assert!(take_live_sync_notices().is_empty());
    }

    #[test]
    fn watermark_only_moves_forward() {
        LAST_SEEN_ID.store(5, Ordering::SeqCst);
        note_own_interaction(3); // lower id: no-op
        assert_eq!(LAST_SEEN_ID.load(Ordering::SeqCst), 5);
        note_own_interaction(9); // higher id: advances
        assert_eq!(LAST_SEEN_ID.load(Ordering::SeqCst), 9);
    }
}

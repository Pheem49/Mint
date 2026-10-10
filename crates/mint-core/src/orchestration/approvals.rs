//! Pending approvals belong to their request and can only be resolved once.
use super::ApprovalOutcome;
use std::collections::HashMap;
use std::sync::Mutex;
use tokio::sync::oneshot;
use tokio_util::sync::CancellationToken;

#[derive(Default)]
pub struct ApprovalRegistry {
    pending: Mutex<HashMap<String, PendingResponse>>,
}

struct PendingResponse {
    sender: oneshot::Sender<ApprovalOutcome>,
    cancellation: Option<CancellationToken>,
}

impl ApprovalRegistry {
    /// Register before publishing. The request owns cleanup, including failed delivery,
    /// unwinding, dropped futures and cooperative cancellation of its calling turn.
    pub async fn request(
        &self,
        emit: impl FnOnce(&str) -> Result<(), String>,
    ) -> Result<ApprovalOutcome, String> {
        self.request_with_cancellation(emit, super::run_control::token())
            .await
    }

    async fn request_with_cancellation(
        &self,
        emit: impl FnOnce(&str) -> Result<(), String>,
        cancellation: Option<CancellationToken>,
    ) -> Result<ApprovalOutcome, String> {
        if cancellation
            .as_ref()
            .is_some_and(CancellationToken::is_cancelled)
        {
            return Ok(ApprovalOutcome::Denied);
        }
        let (tx, rx) = oneshot::channel();
        let token = uuid::Uuid::new_v4().to_string();
        self.pending.lock().unwrap().insert(
            token.clone(),
            PendingResponse {
                sender: tx,
                cancellation: cancellation.clone(),
            },
        );
        let _ownership = PendingApproval {
            registry: self,
            token: token.clone(),
        };
        emit(&token)?;
        let result = if let Some(cancellation) = cancellation {
            tokio::select! {
                biased;
                _ = cancellation.cancelled() => ApprovalOutcome::Denied,
                response = rx => {
                    if cancellation.is_cancelled() { ApprovalOutcome::Denied }
                    else { response.unwrap_or(ApprovalOutcome::Denied) }
                },
            }
        } else {
            rx.await.unwrap_or(ApprovalOutcome::Denied)
        };
        Ok(result)
    }

    /// Synchronous bridge for tool callbacks on a multithread Tokio runtime.
    pub fn request_blocking(
        &self,
        emit: impl FnOnce(&str) -> Result<(), String>,
    ) -> Result<ApprovalOutcome, String> {
        let cancellation = super::run_control::token();
        tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current()
                .block_on(self.request_with_cancellation(emit, cancellation))
        })
    }

    /// Consume once. Blank feedback declines; non-empty feedback takes precedence.
    /// Tokens from cancelled requests are rejected before their wait resumes.
    pub fn resolve(&self, token: &str, approved: bool, answer: Option<String>) -> bool {
        let Some(response) = self.pending.lock().unwrap().remove(token) else {
            return false;
        };
        if response
            .cancellation
            .as_ref()
            .is_some_and(CancellationToken::is_cancelled)
        {
            return false;
        }
        let outcome = match answer {
            Some(answer) if answer.trim().is_empty() => ApprovalOutcome::Denied,
            Some(answer) => ApprovalOutcome::Intercepted(answer),
            None if approved => ApprovalOutcome::Approved,
            None => ApprovalOutcome::Denied,
        };
        let _ = response.sender.send(outcome);
        true
    }
}

struct PendingApproval<'a> {
    registry: &'a ApprovalRegistry,
    token: String,
}

impl Drop for PendingApproval<'_> {
    fn drop(&mut self) {
        self.registry.pending.lock().unwrap().remove(&self.token);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::time::Duration;

    #[tokio::test]
    async fn approval_answers_have_one_meaning_on_every_surface() {
        for (approved, answer, expected) in [
            (true, None, ApprovalOutcome::Approved),
            (false, None, ApprovalOutcome::Denied),
            (true, Some(""), ApprovalOutcome::Denied),
            (true, Some(" \t\n"), ApprovalOutcome::Denied),
            (false, Some(""), ApprovalOutcome::Denied),
            (
                true,
                Some("  revise the plan  "),
                ApprovalOutcome::Intercepted("  revise the plan  ".into()),
            ),
            (
                false,
                Some("use another file"),
                ApprovalOutcome::Intercepted("use another file".into()),
            ),
        ] {
            let registry = ApprovalRegistry::default();
            let mut token = String::new();
            let outcome = registry
                .request(|emitted| {
                    token = emitted.to_owned();
                    assert!(registry.resolve(emitted, approved, answer.map(str::to_owned)));
                    Ok(())
                })
                .await
                .unwrap();
            assert_eq!(outcome, expected);
            assert!(
                !registry.resolve(&token, true, None),
                "a consumed token was replayed"
            );
        }
    }

    #[tokio::test]
    async fn approval_emit_failure_removes_the_registered_token() {
        let registry = ApprovalRegistry::default();
        let mut token = String::new();
        let result = registry
            .request(|emitted| {
                token = emitted.to_owned();
                Err("renderer disconnected".into())
            })
            .await;
        assert_eq!(result.unwrap_err(), "renderer disconnected");
        assert!(
            !registry.resolve(&token, true, None),
            "emit failure left a live token"
        );
    }

    #[tokio::test]
    async fn approval_dropped_request_rejects_a_late_response() {
        let registry = Arc::new(ApprovalRegistry::default());
        let pending = registry.clone();
        let (tx, rx) = oneshot::channel();
        let task = tokio::spawn(async move {
            pending
                .request(|token| {
                    tx.send(token.to_owned()).unwrap();
                    Ok(())
                })
                .await
        });
        let token = rx.await.unwrap();
        task.abort();
        assert!(task.await.unwrap_err().is_cancelled());
        assert!(
            !registry.resolve(&token, true, None),
            "a dropped request kept its token"
        );
    }

    #[tokio::test]
    async fn approval_panicking_emitter_releases_ownership() {
        let registry = Arc::new(ApprovalRegistry::default());
        let pending = registry.clone();
        let (tx, rx) = oneshot::channel();
        let task = tokio::spawn(async move {
            pending
                .request(|token| {
                    tx.send(token.to_owned()).unwrap();
                    panic!("emitter panicked")
                })
                .await
        });
        let token = rx.await.unwrap();
        assert!(task.await.unwrap_err().is_panic());
        assert!(!registry.resolve(&token, true, None));
    }

    #[tokio::test]
    async fn approval_cancelled_turn_stops_waiting_and_rejects_late_response() {
        let registry = Arc::new(ApprovalRegistry::default());
        let pending = registry.clone();
        let chat = uuid::Uuid::new_v4().to_string();
        let task_chat = chat.clone();
        let (tx, rx) = oneshot::channel();
        let mut task = tokio::spawn(async move {
            super::super::run_control::scope(&task_chat, async {
                pending
                    .request(|token| {
                        tx.send(token.to_owned()).unwrap();
                        Ok(())
                    })
                    .await
            })
            .await
        });
        let token = rx.await.unwrap();
        assert!(crate::cancel_agent(&chat));
        let result = tokio::time::timeout(Duration::from_secs(5), &mut task).await;
        if result.is_err() {
            task.abort();
            let _ = task.await;
        }
        assert!(
            matches!(result, Ok(Ok(Ok(ApprovalOutcome::Denied)))),
            "cancellation did not release approval wait: {result:?}"
        );
        assert!(!registry.resolve(&token, true, None));
    }

    #[tokio::test]
    async fn approval_already_cancelled_turn_cannot_publish_a_request() {
        let registry = ApprovalRegistry::default();
        let chat = uuid::Uuid::new_v4().to_string();
        super::super::run_control::scope(&chat, async {
            assert!(crate::cancel_agent(&chat));
            let result = registry
                .request(|_| panic!("cancelled turn emitted an approval"))
                .await;
            assert_eq!(result.unwrap(), ApprovalOutcome::Denied);
        })
        .await;
    }

    #[tokio::test]
    async fn approval_cancellation_wins_over_a_simultaneous_response() {
        let registry = ApprovalRegistry::default();
        let chat = uuid::Uuid::new_v4().to_string();
        super::super::run_control::scope(&chat, async {
            let result = registry
                .request(|token| {
                    assert!(crate::cancel_agent(&chat));
                    assert!(
                        !registry.resolve(token, true, None),
                        "a cancelled token was still accepted"
                    );
                    Ok(())
                })
                .await;
            assert_eq!(result.unwrap(), ApprovalOutcome::Denied);
        })
        .await;
    }

    #[tokio::test]
    async fn approval_old_request_cleanup_preserves_its_replacement() {
        let registry = Arc::new(ApprovalRegistry::default());
        let (old_tx, old_rx) = oneshot::channel();
        let old_registry = registry.clone();
        let old = tokio::spawn(async move {
            old_registry
                .request(|token| {
                    old_tx.send(token.to_owned()).unwrap();
                    Ok(())
                })
                .await
        });
        let old_token = old_rx.await.unwrap();
        let (new_tx, new_rx) = oneshot::channel();
        let new_registry = registry.clone();
        let next = tokio::spawn(async move {
            new_registry
                .request(|token| {
                    new_tx.send(token.to_owned()).unwrap();
                    Ok(())
                })
                .await
        });
        let new_token = new_rx.await.unwrap();
        assert_ne!(old_token, new_token);
        old.abort();
        let _ = old.await;
        assert!(registry.resolve(&new_token, true, None));
        assert_eq!(next.await.unwrap().unwrap(), ApprovalOutcome::Approved);
        assert!(!registry.resolve(&old_token, true, None));
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn approval_blocking_callback_keeps_the_calling_turn_cancellable() {
        let registry = Arc::new(ApprovalRegistry::default());
        let pending = registry.clone();
        let chat = uuid::Uuid::new_v4().to_string();
        let task_chat = chat.clone();
        let (tx, rx) = oneshot::channel();
        let mut task = tokio::spawn(async move {
            super::super::run_control::scope(&task_chat, async {
                pending.request_blocking(|token| {
                    tx.send(token.to_owned()).unwrap();
                    Ok(())
                })
            })
            .await
        });
        let token = rx.await.unwrap();
        assert!(crate::cancel_agent(&chat));
        let result = tokio::time::timeout(Duration::from_secs(5), &mut task).await;
        if result.is_err() {
            registry.resolve(&token, false, None);
            let _ = task.await;
        }
        assert!(
            matches!(result, Ok(Ok(Ok(ApprovalOutcome::Denied)))),
            "blocking approval ignored cancellation: {result:?}"
        );
        assert!(!registry.resolve(&token, true, None));
    }
}

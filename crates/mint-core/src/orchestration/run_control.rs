//! A cancellation belongs to a run, never to a reusable conversation ID.
use std::{
    collections::HashMap,
    future::Future,
    sync::{LazyLock, Mutex},
};
use tokio_util::sync::CancellationToken;

struct Context {
    chat: String,
    token: CancellationToken,
}
tokio::task_local! { static CURRENT: Context; }
static RUNS: LazyLock<Mutex<HashMap<String, (String, CancellationToken)>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

struct Registration {
    id: String,
    token: CancellationToken,
}
impl Drop for Registration {
    fn drop(&mut self) {
        RUNS.lock().unwrap().remove(&self.id);
        self.token.cancel();
    }
}

pub(crate) async fn scope<F: Future>(chat: &str, future: F) -> F::Output {
    let token = CURRENT
        .try_with(|parent| {
            if chat.starts_with(&format!("{}::subagent::", parent.chat)) {
                parent.token.child_token()
            } else {
                CancellationToken::new()
            }
        })
        .unwrap_or_default();
    let id = uuid::Uuid::new_v4().to_string();
    RUNS.lock()
        .unwrap()
        .insert(id.clone(), (chat.into(), token.clone()));
    let _registration = Registration {
        id,
        token: token.clone(),
    };
    CURRENT
        .scope(
            Context {
                chat: chat.into(),
                token,
            },
            future,
        )
        .await
}

pub(crate) fn token() -> Option<CancellationToken> {
    CURRENT.try_with(|c| c.token.clone()).ok()
}
pub(crate) fn cancelled() -> bool {
    token().is_some_and(|t| t.is_cancelled())
}
pub(crate) fn check() -> Result<(), super::OrchestrationError> {
    if cancelled() {
        Err(super::OrchestrationError::Agent(
            "Agent turn canceled by user".into(),
        ))
    } else {
        Ok(())
    }
}
pub(crate) async fn run<T, E: Into<super::OrchestrationError>>(
    future: impl Future<Output = Result<T, E>>,
) -> Result<T, super::OrchestrationError> {
    let Some(token) = token() else {
        return future.await.map_err(Into::into);
    };
    tokio::select! {
        biased;
        _=token.cancelled()=>Err(super::OrchestrationError::Agent("Agent turn canceled by user".into())),
        result=future=>{check()?;result.map_err(Into::into)},
    }
}
pub(crate) fn cancel(chat: &str) -> usize {
    let prefix = format!("{chat}::subagent::");
    let runs = RUNS.lock().unwrap();
    let mut count = 0;
    for (name, token) in runs.values() {
        if name == chat || name.starts_with(&prefix) {
            token.cancel();
            count += 1;
        }
    }
    count
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn review_cancelled_turn_cannot_resume_in_a_fresh_mcp_scope_but_a_new_turn_can() {
        let chat = format!("run-control-{}", uuid::Uuid::new_v4());
        scope(&chat, async {
            assert!(crate::cancel_agent(&chat));
            let result = crate::mcp::catalog::describe_tool(
                &crate::MintConfig::default(),
                "not-configured",
                "move",
                &chat,
            )
            .await;
            assert!(
                matches!(result, Err(crate::mcp::McpError::Cancelled)),
                "new MCP scopes must honor the canceled turn"
            );
        })
        .await;
        scope(&chat, async {
            assert!(check().is_ok(), "new turn inherited stale cancellation");
            let result = crate::mcp::catalog::describe_tool(
                &crate::MintConfig::default(),
                "not-configured",
                "move",
                &chat,
            )
            .await;
            assert!(matches!(
                result,
                Err(crate::mcp::McpError::MissingServer(_))
            ));
        })
        .await;
    }

    #[tokio::test]
    async fn review_parent_cancellation_reaches_late_children_and_child_cancel_is_scoped() {
        let chat = format!("parent-control-{}", uuid::Uuid::new_v4());
        scope(&chat, async {
            let child = format!("{chat}::subagent::one");
            scope(&child, async {
                assert!(crate::cancel_agent(&child));
                assert!(check().is_err());
            })
            .await;
            assert!(check().is_ok(), "canceling a child canceled its parent");
            assert!(crate::cancel_agent(&chat));
            scope(&format!("{chat}::subagent::late"), async {
                assert!(check().is_err());
            })
            .await;
            scope("unrelated-run", async {
                assert!(check().is_ok());
            })
            .await;
        })
        .await;
    }

    #[tokio::test]
    async fn review_old_run_cleanup_does_not_cancel_or_unregister_its_replacement() {
        let chat = format!("overlap-control-{}", uuid::Uuid::new_v4());
        let old_chat = chat.clone();
        let (ready_tx, ready_rx) = tokio::sync::oneshot::channel();
        let (done_tx, done_rx) = tokio::sync::oneshot::channel();
        let old = tokio::spawn(async move {
            scope(&old_chat, async {
                ready_tx.send(()).unwrap();
                done_rx.await.unwrap();
            })
            .await
        });
        ready_rx.await.unwrap();
        assert!(crate::cancel_agent(&chat));
        scope(&chat, async {
            done_tx.send(()).unwrap();
            old.await.unwrap();
            assert!(check().is_ok(), "old cleanup canceled a new turn");
            assert_eq!(cancel(&chat), 1, "replacement registration was lost");
            assert!(check().is_err());
        })
        .await;
    }
}

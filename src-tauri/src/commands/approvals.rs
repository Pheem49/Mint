//! Native event delivery for the shared approval lifecycle.
use mint_core::{AgentApproval, ApprovalOutcome};
use tauri::{AppHandle, Emitter, Manager};

pub type ApprovalsState = mint_core::ApprovalRegistry;

pub(crate) fn callback(
    app: AppHandle,
    chat_id: String,
) -> impl FnMut(&AgentApproval) -> Result<ApprovalOutcome, String> + Send {
    move |approval| {
        app.state::<ApprovalsState>().request_blocking(|token| {
            app.emit(
                "tool-approval-requested",
                serde_json::json!({ "token": token, "approval": approval, "chatId": chat_id }),
            )
            .map_err(|error| error.to_string())
        })
    }
}

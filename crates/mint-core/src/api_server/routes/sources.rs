use super::super::*;
use super::RequestCtx;
use crate::agent::memory::SourceAttachment;
use tokio::net::TcpStream;

pub(in crate::api_server) async fn execute(ctx: RequestCtx<'_>, socket: TcpStream) {
    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct SaveRequest {
        chat_id: String,
        interaction_id: i64,
        attachments: Vec<SourceAttachment>,
    }
    let request = if ctx.method == "POST" {
        serde_json::from_str::<SaveRequest>(ctx.body).ok()
    } else {
        None
    };
    if ctx.method == "POST" && request.is_none() {
        send_json_response(
            socket,
            "400 Bad Request",
            "{\"error\":\"Invalid source metadata\"}",
        )
        .await;
        return;
    }
    let chat_id = request
        .as_ref()
        .map(|r| r.chat_id.clone())
        .or_else(|| query_param(ctx.query, "chatId"))
        .unwrap_or_default();
    let owner = match background_request_owner(ctx.request_str) {
        Ok(owner) => owner,
        Err(message) => {
            send_json_response(
                socket,
                "401 Unauthorized",
                &json!({"error": message}).to_string(),
            )
            .await;
            return;
        }
    };
    if chat_id.trim().is_empty() {
        send_json_response(
            socket,
            "400 Bad Request",
            "{\"error\":\"chatId is required\"}",
        )
        .await;
        return;
    }
    if !crate::bg_shell::bind_chat_owner(&chat_id, owner) {
        send_json_response(
            socket,
            "403 Forbidden",
            "{\"error\":\"Conversation belongs to another user\"}",
        )
        .await;
        return;
    }
    let result = tokio::task::spawn_blocking(move || -> Result<Value, String> {
        let store = MemoryStore::open_default().map_err(|e| e.to_string())?;
        if let Some(request) = request {
            store.save_source_attachments(
                &chat_id,
                request.interaction_id,
                &request.attachments,
            )?;
            Ok(json!({"status":"ok"}))
        } else {
            serde_json::to_value(
                store
                    .conversation_sources(&chat_id)
                    .map_err(|e| e.to_string())?,
            )
            .map_err(|e| e.to_string())
        }
    })
    .await;
    match result {
        Ok(Ok(value)) => send_json_response(socket, "200 OK", &value.to_string()).await,
        Ok(Err(error)) => {
            send_json_response(
                socket,
                "400 Bad Request",
                &json!({"error": error}).to_string(),
            )
            .await
        }
        Err(_) => {
            send_json_response(
                socket,
                "500 Internal Server Error",
                "{\"error\":\"Could not load session sources\"}",
            )
            .await
        }
    }
}

use super::RequestCtx;
use crate::api_server::send_json_response;
use serde_json::json;
use tokio::net::TcpStream;

pub(in crate::api_server) async fn execute(ctx: RequestCtx<'_>, socket: TcpStream) {
    let op = match serde_json::from_str::<crate::mods::Operation>(ctx.body) {
        Ok(op) => op,
        Err(e) => {
            send_json_response(
                socket,
                "400 Bad Request",
                &json!({"error":e.to_string()}).to_string(),
            )
            .await;
            return;
        }
    };
    let result = crate::mods::operate_async(op).await;
    match result {
        Ok(value) => send_json_response(socket, "200 OK", &value.to_string()).await,
        Err(error) => {
            send_json_response(
                socket,
                "400 Bad Request",
                &json!({"error":error}).to_string(),
            )
            .await
        }
    }
}

use super::{super::*, RequestCtx};
use tokio::net::TcpStream;

pub(in crate::api_server) async fn execute(ctx: RequestCtx<'_>, socket: TcpStream) {
    // Preview URLs deliberately bind only to loopback, matching native Desktop.
    if socket
        .peer_addr()
        .map_or(true, |address| !address.ip().is_loopback())
    {
        send_json_response(socket, "403 Forbidden", "{\"message\":\"HTML preview requires a browser on the same machine as the Mint backend.\"}").await;
        return;
    }
    let owner = match background_request_owner(ctx.request_str) {
        Ok(owner) => owner,
        Err(message) => {
            send_json_response(
                socket,
                "401 Unauthorized",
                &serde_json::json!({"message": message}).to_string(),
            )
            .await;
            return;
        }
    };
    let result = async {
        let body: serde_json::Value = serde_json::from_str(ctx.body).map_err(|e| e.to_string())?;
        let root = body["root"]
            .as_str()
            .ok_or("Missing workspace root")?
            .to_string();
        let path = body["relativePath"]
            .as_str()
            .ok_or("Missing HTML path")?
            .to_string();
        let mint = body["mintBrowser"].as_bool().unwrap_or(false);
        let mut config = load_config().map_err(|e| e.to_string())?;
        let worker_config = config.clone();
        let preview = tokio::task::spawn_blocking(move || {
            crate::system::html_preview::start(
                std::path::Path::new(&root),
                &path,
                owner,
                &worker_config,
            )
        })
        .await
        .map_err(|e| e.to_string())??;
        let url = preview["url"].as_str().ok_or("Preview URL unavailable")?;
        reqwest::Client::new()
            .head(url)
            .timeout(std::time::Duration::from_secs(2))
            .send()
            .await
            .map_err(|e| e.to_string())?
            .error_for_status()
            .map_err(|e| e.to_string())?;
        if mint {
            crate::enable_browser_tools(&mut config);
            crate::browser::spawn_automation_browser_with_url(&config, Some(url)).await?;
        }
        Ok::<_, String>(preview)
    }
    .await;
    match result {
        Ok(value) => send_json_response(socket, "200 OK", &value.to_string()).await,
        Err(message) => {
            send_json_response(
                socket,
                "400 Bad Request",
                &serde_json::json!({"message": message}).to_string(),
            )
            .await
        }
    }
}

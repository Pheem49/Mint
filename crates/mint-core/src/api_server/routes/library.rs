use super::{
    super::{percent_decode, query_param, send_json_response},
    RequestCtx,
};
use crate::library::{LibraryQuery, LibraryStore};
use tokio::{io::AsyncWriteExt, net::TcpStream};

pub(in crate::api_server) async fn execute(ctx: RequestCtx<'_>, mut socket: TcpStream) {
    let method = ctx.method.to_owned();
    let route = ctx.route.to_owned();
    let query = LibraryQuery {
        kind: query_param(ctx.query, "kind"),
        search: query_param(ctx.query, "search"),
        chat_id: query_param(ctx.query, "chatId"),
        trashed: query_param(ctx.query, "trashed").is_some_and(|s| s == "true"),
        cursor: query_param(ctx.query, "cursor"),
    };
    // SQLite, migration, and disk IO never block the async API executor.
    enum Reply {
        Json(String),
        Bytes(String, Vec<u8>),
    }
    let result = tokio::task::spawn_blocking(move || -> Result<Reply, String> {
        let store = LibraryStore::open_default().map_err(|e| e.to_string())?;
        let tail = route
            .strip_prefix("/api/library")
            .unwrap_or("")
            .trim_start_matches('/');
        if method == "GET" && tail.is_empty() {
            return serde_json::to_string(&store.list(&query).map_err(|e| e.to_string())?)
                .map(Reply::Json)
                .map_err(|e| e.to_string());
        }
        let (id, action) = tail.split_once('/').unwrap_or((tail, ""));
        let id = percent_decode(id);
        match (method.as_str(), action) {
            ("GET", "") => serde_json::to_string(&store.get(&id).map_err(|e| e.to_string())?)
                .map(Reply::Json)
                .map_err(|e| e.to_string()),
            ("GET", "content" | "thumbnail") => {
                let thumb = action == "thumbnail";
                let (item, bytes) = store.read(&id, thumb).map_err(|e| e.to_string())?;
                let mime = if thumb {
                    "image/png".to_owned()
                } else {
                    item.mime_type
                };
                Ok(Reply::Bytes(mime, bytes))
            }
            ("DELETE", "") => {
                store.remove(&id).map_err(|e| e.to_string())?;
                Ok(Reply::Json("{}".into()))
            }
            ("POST", "trash" | "restore") => {
                store
                    .set_trashed(&id, action == "trash")
                    .map_err(|e| e.to_string())?;
                Ok(Reply::Json("{}".into()))
            }
            _ => Err("Unknown Library operation".into()),
        }
    })
    .await;
    match result {
        Ok(Ok(Reply::Json(json))) => send_json_response(socket, "200 OK", &json).await,
        Ok(Ok(Reply::Bytes(mime, bytes))) => {
            // Content is downloaded through authenticated fetch; never execute uploaded HTML on this origin.
            let mime = if mime
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"/.-+;= ".contains(&b))
            {
                mime
            } else {
                "application/octet-stream".into()
            };
            let headers = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: {mime}\r\nContent-Length: {}\r\nContent-Disposition: attachment\r\nX-Content-Type-Options: nosniff\r\nCache-Control: no-store\r\nAccess-Control-Allow-Origin: *\r\nConnection: close\r\n\r\n",
                bytes.len()
            );
            let _ = socket.write_all(headers.as_bytes()).await;
            let _ = socket.write_all(&bytes).await;
        }
        other => {
            let message = match other {
                Ok(Err(error)) => error,
                Err(error) => error.to_string(),
                _ => unreachable!(),
            };
            send_json_response(
                socket,
                "400 Bad Request",
                &serde_json::json!({"message":message}).to_string(),
            )
            .await;
        }
    }
}

use super::super::*;
use super::RequestCtx;
use std::path::Path;
use tokio::net::TcpStream;

pub(in crate::api_server) async fn execute(ctx: RequestCtx<'_>, socket: TcpStream) {
    let owner = match background_request_owner(ctx.request_str) {
        Ok(owner) => owner,
        Err(message) => {
            send_json_response(
                socket,
                "401 Unauthorized",
                &json!({"message": message}).to_string(),
            )
            .await;
            return;
        }
    };
    let config = match load_config() {
        Ok(config) => config,
        Err(error) => {
            send_json_response(
                socket,
                "500 Internal Server Error",
                &json!({"message": error.to_string()}).to_string(),
            )
            .await;
            return;
        }
    };
    execute_for(ctx, socket, owner, &config).await;
}

async fn execute_for(
    ctx: RequestCtx<'_>,
    socket: TcpStream,
    owner: Option<String>,
    config: &MintConfig,
) {
    if ctx.route == "/api/background-jobs" && ctx.method == "GET" {
        let mut workspace = ctx
            .query
            .split('&')
            .find_map(|part| part.strip_prefix("workspace="))
            .map(percent_decode);
        if let Some(path) = workspace.as_deref() {
            match crate::assert_path_capability(Path::new(path), crate::Capability::Read, config) {
                Ok(path) => workspace = Some(path.to_string_lossy().into_owned()),
                Err(_) => {
                    send_json_response(
                        socket,
                        "403 Forbidden",
                        "{\"message\":\"Workspace access denied\"}",
                    )
                    .await;
                    return;
                }
            }
        }
        let jobs: Vec<_> = crate::bg_shell::list_json(&owner, workspace.as_deref().map(Path::new))
            .into_iter()
            .filter(|job| {
                job["workspacePath"].as_str().is_some_and(|path| {
                    crate::assert_path_capability(Path::new(path), crate::Capability::Read, &config)
                        .is_ok()
                })
            })
            .collect();
        send_json_response(
            socket,
            "200 OK",
            &serde_json::json!({"jobs": jobs}).to_string(),
        )
        .await;
        return;
    }
    let tail = ctx
        .route
        .strip_prefix("/api/background-jobs/")
        .unwrap_or("");
    let (id, stop) = tail
        .strip_suffix("/stop")
        .map_or((tail, false), |id| (id, true));
    if id.is_empty() || id.contains('/') || !crate::bg_shell::accessible(id, &owner) {
        send_json_response(
            socket,
            "404 Not Found",
            "{\"message\":\"Background job not found\"}",
        )
        .await;
        return;
    }
    let job = match crate::bg_shell::job_json(id, !stop) {
        Ok(job) => job,
        Err(_) => {
            send_json_response(
                socket,
                "404 Not Found",
                "{\"message\":\"Background job not found\"}",
            )
            .await;
            return;
        }
    };
    let capability = if stop && job["managedService"] != true {
        crate::Capability::Write
    } else {
        crate::Capability::Read
    };
    if job["workspacePath"].as_str().is_none_or(|path| {
        crate::assert_path_capability(Path::new(path), capability, &config).is_err()
    }) {
        send_json_response(
            socket,
            "403 Forbidden",
            "{\"message\":\"Workspace access denied\"}",
        )
        .await;
        return;
    }
    if ctx.method == "GET" && !stop {
        send_json_response(socket, "200 OK", &job.to_string()).await;
    } else if ctx.method == "POST" && stop {
        match crate::bg_shell::kill_job(id) {
            Ok(_) => {
                send_json_response(
                    socket,
                    "200 OK",
                    &crate::bg_shell::job_json(id, false)
                        .unwrap_or(job)
                        .to_string(),
                )
                .await
            }
            Err(error) => {
                send_json_response(
                    socket,
                    "500 Internal Server Error",
                    &serde_json::json!({"message": error.to_string()}).to_string(),
                )
                .await
            }
        }
    } else {
        send_json_response(
            socket,
            "405 Method Not Allowed",
            "{\"message\":\"Method not allowed\"}",
        )
        .await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bg_shell;
    use tokio::io::AsyncReadExt;

    async fn request(
        method: &str,
        route: &str,
        query: &str,
        owner: Option<String>,
        config: &MintConfig,
    ) -> (u16, Value) {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let (client, server) = tokio::join!(TcpStream::connect(address), listener.accept());
        let mut client = client.unwrap();
        let socket = server.unwrap().0;
        let ctx = RequestCtx {
            method,
            route,
            query,
            body: "",
            request_str: "",
            request_bytes: &[],
            header_end: 0,
            auth_label: "test".into(),
        };
        let (_, response) = tokio::join!(execute_for(ctx, socket, owner, config), async move {
            let mut bytes = Vec::new();
            client.read_to_end(&mut bytes).await.unwrap();
            String::from_utf8(bytes).unwrap()
        });
        let status = response.split_whitespace().nth(1).unwrap().parse().unwrap();
        let body = response.split_once("\r\n\r\n").unwrap().1;
        (status, serde_json::from_str(body).unwrap())
    }
    struct Cleanup(Vec<String>);
    impl Drop for Cleanup {
        fn drop(&mut self) {
            for id in &self.0 {
                let _ = bg_shell::kill_job(id);
            }
        }
    }

    #[tokio::test]
    async fn endpoints_filter_owners_check_capabilities_and_stop_one_job() {
        let config = MintConfig {
            safety_enabled: false,
            sandbox_mode: "off".into(),
            ..Default::default()
        };
        let root = std::env::current_dir().unwrap();
        let owner = Some(uuid::Uuid::new_v4().to_string());
        let other = Some(uuid::Uuid::new_v4().to_string());
        let start = |owner| {
            bg_shell::with_context(
                bg_shell::JobContext {
                    workspace: root.clone(),
                    chat_id: Some("api-test".into()),
                    owner,
                },
                || bg_shell::start_background(&root, &config, "printf ready; sleep 30"),
            )
            .unwrap()
        };
        let first = start(owner.clone());
        let second = start(other.clone());
        let _cleanup = Cleanup(vec![first.id.clone(), second.id.clone()]);
        let route = format!("/api/background-jobs/{}", first.id);
        let (status, list) =
            request("GET", "/api/background-jobs", "", owner.clone(), &config).await;
        assert_eq!(status, 200);
        assert_eq!(list["jobs"].as_array().unwrap().len(), 1);
        assert_eq!(list["jobs"][0]["id"], first.id);
        assert_eq!(
            request("GET", &route, "", other.clone(), &config).await.0,
            404
        );
        assert_eq!(
            request(
                "GET",
                "/api/background-jobs/bg-stale-1",
                "",
                owner.clone(),
                &config
            )
            .await
            .0,
            404
        );
        let (status, output) = request("GET", &route, "", owner.clone(), &config).await;
        assert_eq!(status, 200);
        assert!(output["stdout"].is_string());
        assert_eq!(
            request("POST", &route, "", owner.clone(), &config).await.0,
            405
        );
        let stop = format!("{route}/stop");
        let deny_write = MintConfig {
            safety_enabled: true,
            allowed_read_paths: vec![root.clone()],
            allowed_write_paths: vec![],
            ..config.clone()
        };
        assert_eq!(
            request("POST", &stop, "", owner.clone(), &deny_write)
                .await
                .0,
            403
        );
        assert_eq!(
            bg_shell::job_json(&first.id, false).unwrap()["status"],
            "running"
        );
        let deny_read = MintConfig {
            safety_enabled: true,
            allowed_read_paths: vec![],
            ..config.clone()
        };
        assert_eq!(
            request("GET", &route, "", owner.clone(), &deny_read)
                .await
                .0,
            403
        );
        let (status, stopped) = request("POST", &stop, "", owner.clone(), &config).await;
        assert_eq!(status, 200);
        assert!(stopped["status"] == "stopping" || stopped["status"] == "stopped");
        assert_eq!(
            bg_shell::job_json(&second.id, false).unwrap()["status"],
            "running"
        );
        assert_eq!(request("POST", &stop, "", owner, &config).await.0, 200);
    }
}

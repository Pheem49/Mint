use super::RequestCtx;
use tokio::net::TcpStream;

use super::super::*;

pub(in crate::api_server) async fn execute(ctx: RequestCtx<'_>, socket: TcpStream) {
    let RequestCtx {
        method,
        route,
        query: _query,
        body,
        request_str: _request_str,
        request_bytes: _request_bytes,
        header_end: _header_end,
        auth_label: _auth_label,
    } = ctx;
    match (method, route) {
        ("GET", "/api/learned-skills") => {
            let mut skills = match MemoryStore::open_default() {
                Ok(m) => m.learned_skills(100).unwrap_or_default(),
                Err(_) => Vec::new(),
            };

            if let Some(home) = dirs::home_dir() {
                let global_agents_path = home.join(".gemini").join("config").join("AGENTS.md");
                crate::skills::load_agent_rules_file(&global_agents_path, &mut skills);

                let global_skills_path = home.join(".config").join("mint").join("mint-skills");
                crate::skills::load_skills_from_dir(&global_skills_path, &mut skills);
            }

            if let Ok(current_dir) = std::env::current_dir() {
                let workspace_agents_path1 = current_dir.join(".agents").join("AGENTS.md");
                crate::skills::load_agent_rules_file(&workspace_agents_path1, &mut skills);
                let workspace_agents_path2 = current_dir.join("AGENTS.md");
                crate::skills::load_agent_rules_file(&workspace_agents_path2, &mut skills);

                let workspace_skills_path1 = current_dir.join(".agents").join("skills");
                crate::skills::load_skills_from_dir(&workspace_skills_path1, &mut skills);
                let workspace_skills_path2 = current_dir.join("skills");
                crate::skills::load_skills_from_dir(&workspace_skills_path2, &mut skills);

                if let Ok(canonical_cwd) = current_dir.canonicalize() {
                    for s in &mut skills {
                        if let Ok(p) = std::path::Path::new(&s.source_path).canonicalize() {
                            if p.starts_with(&canonical_cwd) {
                                s.is_workspace = true;
                            }
                        }
                    }
                }
            }

            let mut unique_skills = std::collections::BTreeMap::new();
            for s in skills {
                unique_skills.insert(s.name.clone(), s);
            }

            let list: Vec<_> = unique_skills.into_values().collect();
            send_json_response(
                socket,
                "200 OK",
                &serde_json::to_string(&list).unwrap_or_default(),
            )
            .await;
            return;
        }

        ("GET", "/api/subagents") => {
            let root = std::env::current_dir().ok();
            let list = crate::subagents::list_subagents(root.as_deref());
            send_json_response(
                socket,
                "200 OK",
                &serde_json::to_string(&list).unwrap_or_default(),
            )
            .await;
        }

        ("POST", "/api/subagents") => {
            match serde_json::from_str::<crate::subagents::SubagentDraft>(body) {
                Ok(draft) => {
                    let root = std::env::current_dir().ok();
                    match crate::subagents::save_subagent(&draft, root.as_deref()) {
                        Ok(saved) => {
                            send_json_response(
                                socket,
                                "200 OK",
                                &serde_json::to_string(&saved).unwrap_or_default(),
                            )
                            .await;
                        }
                        Err(err) => {
                            let err_msg = json!({ "error": err }).to_string();
                            send_json_response(socket, "400 Bad Request", &err_msg).await;
                        }
                    }
                }
                Err(_) => {
                    send_json_response(
                        socket,
                        "400 Bad Request",
                        "{\"error\":\"Invalid request body.\"}",
                    )
                    .await;
                }
            }
        }

        ("DELETE", route) if route.starts_with("/api/subagents/") => {
            let source_path = percent_decode(route.trim_start_matches("/api/subagents/"));
            match crate::subagents::delete_subagent(&source_path) {
                Ok(()) => {
                    send_json_response(socket, "200 OK", "{\"status\":\"ok\"}").await;
                }
                Err(err) => {
                    let err_msg = json!({ "error": err }).to_string();
                    send_json_response(socket, "400 Bad Request", &err_msg).await;
                }
            }
        }

        _ => unreachable!(
            "api_server routed an unhandled route into routes::skills_subagents::execute: {method} {route}"
        ),
    }
}

use super::*;
use crate::mcp_result::ToolStatus;
use serde_json::json;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

fn config(script: String) -> (MintConfig, String) {
    let name = format!("review-{}", uuid::Uuid::new_v4());
    let mut cfg = MintConfig::default();
    cfg.extra.insert(
        "mcpServers".into(),
        json!({name.clone():{"command":"python3","args":["-u","-c",script],"timeoutSecs":5}}),
    );
    cfg.extra
        .insert("allowedMcpTools".into(), json!({name.clone():["*"]}));
    (cfg, name)
}

#[tokio::test]
async fn review_cancel_during_discovery_never_dispatches_a_command() {
    // Avoid scanning the real workspace (including its build artifacts) before
    // the mocked model reaches discovery, which is the behavior under test.
    let workspace =
        std::env::temp_dir().join(format!("mint-cancel-workspace-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&workspace).unwrap();
    let marker = std::env::temp_dir().join(format!("mint-review-list-{}", uuid::Uuid::new_v4()));
    let calls = marker.with_extension("calls");
    let script = format!(
        r#"
import sys,json
lists=0
for line in sys.stdin:
 r=json.loads(line);m=r.get('method')
 if m=='initialize': result={{}}
 elif m=='tools/list':
  lists+=1
  if lists==1:
   open({:?},'w').close()
   continue
  result={{'tools':[{{'name':'move','inputSchema':{{'type':'object'}}}}]}}
 elif m=='tools/call':
  open({:?},'a').write('command\n')
  result={{'content':[{{'type':'text','text':'moved'}}]}}
 else: continue
 print(json.dumps({{'jsonrpc':'2.0','id':r['id'],'result':result}}),flush=True)
"#,
        marker.to_string_lossy(),
        calls.to_string_lossy()
    );
    let (mut cfg, name) = config(script);
    cfg.ai_provider = "ollama".into();
    cfg.ollama_model = "qwen2.5".into();
    cfg.semantic_fact_recall = false;
    cfg.memory_recall = false;
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    cfg.ollama_host = format!("http://{}", listener.local_addr().unwrap());
    let server = name.clone();
    let provider = tokio::spawn(async move {
        for step in 0..4 {
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut bytes = Vec::new();
            let mut chunk = [0; 4096];
            let (end, length) = loop {
                let n = socket.read(&mut chunk).await.unwrap();
                if n == 0 {
                    return;
                }
                bytes.extend_from_slice(&chunk[..n]);
                if let Some(end) = bytes.windows(4).position(|w| w == b"\r\n\r\n") {
                    let header = String::from_utf8_lossy(&bytes[..end]);
                    let length = header
                        .lines()
                        .find_map(|l| {
                            l.to_ascii_lowercase()
                                .strip_prefix("content-length:")
                                .and_then(|v| v.trim().parse::<usize>().ok())
                        })
                        .unwrap();
                    break (end + 4, length);
                }
            };
            while bytes.len() < end + length {
                let n = socket.read(&mut chunk).await.unwrap();
                if n == 0 {
                    return;
                }
                bytes.extend_from_slice(&chunk[..n]);
            }
            let (action, args) = if step < 3 {
                (
                    "mcp_tool",
                    json!({"server":server,"tool":"move","arguments":{}}),
                )
            } else {
                (
                    "finish",
                    json!({"summary":"done","verification":"mock command observed"}),
                )
            };
            let body = format!(
                "{}\n",
                json!({"model":"qwen2.5","message":{"role":"assistant","content":"","tool_calls":[{"function":{"name":action,"arguments":args}}]},"done":true})
            );
            let _=socket.write_all(format!("HTTP/1.1 200 OK\r\nContent-Type: application/x-ndjson\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",body.len()).as_bytes()).await;
        }
    });
    let chat = format!("review-cancel-{}", uuid::Uuid::new_v4());
    let cancel_chat = chat.clone();
    let observed = marker.clone();
    let canceller = tokio::spawn(async move {
        tokio::time::timeout(std::time::Duration::from_secs(5), async {
            while !observed.exists() {
                tokio::time::sleep(std::time::Duration::from_millis(5)).await;
            }
        })
        .await
        .unwrap();
        assert!(crate::cancel_agent(&cancel_chat));
    });
    let result = tokio::time::timeout(
        std::time::Duration::from_secs(10),
        orchestrate_agent_loop(
            &cfg,
            "Use the simulated move tool",
            &workspace,
            None,
            None,
            None,
            Some(&chat),
            None,
            None,
            None,
            true,
            false,
            |_| Ok(ApprovalOutcome::Denied),
            |_| {},
            |_| {},
        ),
    )
    .await;
    canceller.await.unwrap();
    provider.abort();
    crate::mcp::close_mcp_session(&name);
    let commands = std::fs::read_to_string(&calls).unwrap_or_default();
    let _ = std::fs::remove_file(marker);
    let _ = std::fs::remove_file(calls);
    let _ = std::fs::remove_dir(workspace);
    let _ = MemoryStore::open_default()
        .unwrap()
        .delete_chat_session(&chat);
    assert!(
        commands.is_empty(),
        "canceled discovery dispatched a command: {commands}"
    );
    assert!(
        matches!(result, Ok(Err(_))),
        "cancellation must interrupt the turn: {result:?}"
    );
}

#[tokio::test]
async fn review_native_approval_failure_is_not_a_successful_tool_result() {
    let script = r#"
import sys,json
for line in sys.stdin:
 r=json.loads(line);m=r.get('method')
 if m=='initialize': result={}
 elif m=='tools/list': result={'tools':[{'name':'move','inputSchema':{'type':'object'}}]}
 elif m=='tools/call': result={'content':[{'type':'text','text':'moved'}]}
 else: continue
 print(json.dumps({'jsonrpc':'2.0','id':r['id'],'result':result}),flush=True)
"#;
    let (mut cfg, name) = config(script.into());
    cfg.extra.remove("allowedMcpTools");
    let decision: AgentDecision = serde_json::from_value(
        json!({"action":"mcp_tool","input":{"server":name,"tool":"move","arguments":{}}}),
    )
    .unwrap();
    for approval in [
        ApprovalOutcome::Denied,
        ApprovalOutcome::Intercepted("Use motor B instead".into()),
    ] {
        let out = execute_tool_outcome(
            Path::new("."),
            &cfg,
            &decision,
            "review-native",
            &mut |_| Ok(approval.clone()),
            &mut |_| {},
            "one",
        )
        .await
        .unwrap();
        assert_eq!(out.status, ToolStatus::Failed);
        let results = vec![ToolResultEntry {
            call_id: "one".into(),
            action: "mcp_tool".into(),
            input: Value::Null,
            text: out.text,
            status: out.status,
        }];
        let mut messages = Vec::new();
        append_native_tool_results(
            &mut messages,
            "",
            &results,
            &Default::default(),
            &Default::default(),
        );
        assert!(
            matches!(
                &messages[1].content[0],
                ContentBlock::ToolResult { is_error: true, .. }
            ),
            "denial/feedback must reach the native model as failure"
        );
    }
    crate::mcp::close_mcp_session(&name);
}

#[tokio::test]
async fn review_successful_mcp_error_text_keeps_native_success_and_breaks_failure_streak() {
    let (cfg, name) = config(r#"
import sys,json
for line in sys.stdin:
 r=json.loads(line);m=r.get('method')
 if m=='initialize': result={}
 elif m=='tools/list': result={'tools':[{'name':'diagnostic','inputSchema':{'type':'object'}}]}
 elif m=='tools/call': result={'content':[{'type':'text','text':'Error: sensor log contains this literal label'}]}
 else: continue
 print(json.dumps({'jsonrpc':'2.0','id':r['id'],'result':result}),flush=True)
"#.into());
    let decision = serde_json::from_value(
        json!({"action":"mcp_tool","input":{"server":name,"tool":"diagnostic","arguments":{}}}),
    )
    .unwrap();
    let out = execute_tool_outcome(
        Path::new("."),
        &cfg,
        &decision,
        "review-success",
        &mut |_| Ok(ApprovalOutcome::Denied),
        &mut |_| {},
        "diagnostic",
    )
    .await
    .unwrap();
    crate::mcp::close_mcp_session(&name);
    assert_eq!(out.status, ToolStatus::Success);
    assert!(out.text.starts_with("Error:"));
    let entry = |text: &str, status| ToolResultEntry {
        call_id: "diagnostic".into(),
        action: "mcp_tool".into(),
        input: json!({}),
        text: text.into(),
        status,
    };
    let results = vec![entry(&out.text, out.status)];
    for success_text in [
        &out.text,
        "User denied diagnostic label",
        "User did not approve diagnostic label",
    ] {
        let mut guard = repeated_failures::RepeatedFailures::default();
        for step in 1..=4 {
            guard
                .observe(step, &mut [entry("User denied", ToolStatus::Failed)])
                .unwrap();
        }
        guard
            .observe(5, &mut [entry(success_text, ToolStatus::Success)])
            .unwrap();
        assert!(
            guard
                .observe(6, &mut [entry("User denied", ToolStatus::Failed)])
                .unwrap()
                .is_empty()
        );
    }
    let mut messages = Vec::new();
    append_native_tool_results(
        &mut messages,
        "",
        &results,
        &Default::default(),
        &Default::default(),
    );
    assert!(matches!(
        &messages[1].content[0],
        ContentBlock::ToolResult {
            is_error: false,
            ..
        }
    ));
}

#[tokio::test]
async fn review_cancellation_while_approval_returns_prevents_command_dispatch() {
    let log = std::env::temp_dir().join(format!("mint-approval-cancel-{}", uuid::Uuid::new_v4()));
    let script = format!(
        r#"
import sys,json
for line in sys.stdin:
 r=json.loads(line);m=r.get('method')
 if m=='initialize': result={{}}
 elif m=='tools/list': result={{'tools':[{{'name':'move','inputSchema':{{'type':'object'}}}}]}}
 elif m=='tools/call':
  open({:?},'a').write('move\n')
  result={{'content':[]}}
 else: continue
 print(json.dumps({{'jsonrpc':'2.0','id':r['id'],'result':result}}),flush=True)
"#,
        log.to_string_lossy()
    );
    let (mut cfg, name) = config(script);
    cfg.extra.remove("allowedMcpTools");
    let chat = format!("review-approval-cancel-{}", uuid::Uuid::new_v4());
    let decision = serde_json::from_value(
        json!({"action":"mcp_tool","input":{"server":name,"tool":"move","arguments":{}}}),
    )
    .unwrap();
    let result = run_control::scope(&chat, async {
        execute_tool_outcome(
            Path::new("."),
            &cfg,
            &decision,
            &chat,
            &mut |_| {
                assert!(crate::cancel_agent(&chat));
                Ok(ApprovalOutcome::Approved)
            },
            &mut |_| {},
            "move",
        )
        .await
    })
    .await;
    crate::mcp::close_mcp_session(&name);
    assert!(result.is_err());
    let sent = std::fs::read_to_string(&log).unwrap_or_default();
    let _ = std::fs::remove_file(log);
    assert!(
        sent.is_empty(),
        "approval raced cancellation and dispatched: {sent}"
    );
}

#[tokio::test]
async fn parallel_read_only_progress_keeps_distinct_call_ids() {
    let root = std::env::temp_dir().join(format!("mint-parallel-review-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&root).unwrap();
    std::fs::write(root.join("same.txt"), "contents").unwrap();
    let decisions: Vec<_> = ["read-a", "read-b"]
        .into_iter()
        .map(|id| {
            (
                id.into(),
                AgentDecision {
                    thought: String::new(),
                    action: "read_file".into(),
                    input: AgentInput {
                        path: "same.txt".into(),
                        ..Default::default()
                    },
                },
            )
        })
        .collect();
    let mut events = Vec::new();
    run_parallel_read_only_batch(
        &decisions,
        1,
        &root,
        &MintConfig::default(),
        "review-ids",
        &mut |_| Ok(ApprovalOutcome::Approved),
        &mut |event| events.push(event),
        &mut BTreeMap::new(),
        &mut Vec::new(),
        &mut std::collections::HashMap::new(),
        &[],
        None,
        false,
    )
    .await;
    for id in ["read-a", "read-b"] {
        assert!(events.iter().any(
            |e| matches!(e, AgentProgress::ToolStart { call_id: Some(found), .. } if found == id)
        ));
        assert!(events.iter().any(
            |e| matches!(e, AgentProgress::ToolEnd { call_id: Some(found), .. } if found == id)
        ));
    }
}

#[tokio::test]
async fn parallel_dispatch_shows_parent_calls_before_provider_finishes() {
    let root = std::env::temp_dir().join(format!("mint-parallel-review-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&root).unwrap();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    // Keep the provider pending: parent tools must already be visible while waiting.
    let cfg = MintConfig {
        ai_provider: "ollama".into(),
        ollama_host: format!("http://{}", listener.local_addr().unwrap()),
        memory_recall: false,
        semantic_fact_recall: false,
        ..Default::default()
    };
    let tasks = vec![
        crate::swarm::SwarmTask {
            role: Some("dispatch-a".into()),
            base: Some("explorer".into()),
            instruction: "inspect".into(),
        },
        crate::swarm::SwarmTask {
            role: Some("dispatch-b".into()),
            base: Some("explorer".into()),
            instruction: "inspect".into(),
        },
    ];
    let mut events = Vec::new();
    let _ = tokio::time::timeout(
        std::time::Duration::from_millis(500),
        dispatch_swarm(
            &root,
            &cfg,
            "review-live-dispatch",
            tasks,
            &mut |_| Ok(ApprovalOutcome::Approved),
            &mut |event| events.push(event),
        ),
    )
    .await;
    for id in ["dispatch-a", "dispatch-b"] {
        assert!(
            events
                .iter()
                .any(|e| matches!(e, AgentProgress::SubagentStarted { subagent, .. } if subagent == id)),
            "missing live {id}"
        );
    }
}

#[test]
fn test_agent_loop_dispatch_swarm_nested_execution() {
    let rt = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
        .unwrap();

    rt.block_on(async {
        let config_dir = std::env::temp_dir().join(format!("mint-cfg-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(config_dir.join("mint")).unwrap();
        unsafe {
            std::env::set_var("XDG_CONFIG_HOME", &config_dir);
            std::env::set_var("NO_PROXY", "127.0.0.1,localhost");
            std::env::set_var("no_proxy", "127.0.0.1,localhost");
        }

        let workspace = std::env::temp_dir().join(format!("mint-swarm-ws-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&workspace).unwrap();

        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();

        let cfg = MintConfig {
            ai_provider: "ollama".into(),
            ollama_host: format!("http://127.0.0.1:{port}"),
            ollama_model: "qwen2.5".into(),
            semantic_fact_recall: false,
            memory_recall: false,
            swarm_max_parallel: 4,
            ..Default::default()
        };

        let provider = tokio::spawn(async move {
            // Step 1: Parent agent requests completion. We return tool_calls: dispatch_agents with 3 tasks.
            // Steps 2, 3, 4: Each of the 3 subagents requests completion. We return finish.
            // Step 5: Parent agent finishes with summary.
            for step in 0..10 {
                let Ok((mut socket, _)) = listener.accept().await else {
                    break;
                };
                let mut bytes = Vec::new();
                let mut chunk = [0; 4096];
                let (end, length) = loop {
                    let n = socket.read(&mut chunk).await.unwrap();
                    if n == 0 {
                        return;
                    }
                    bytes.extend_from_slice(&chunk[..n]);
                    if let Some(end) = bytes.windows(4).position(|w| w == b"\r\n\r\n") {
                        let header = String::from_utf8_lossy(&bytes[..end]);
                        let length = header
                            .lines()
                            .find_map(|l| {
                                l.to_ascii_lowercase()
                                    .strip_prefix("content-length:")
                                    .and_then(|v| v.trim().parse::<usize>().ok())
                            })
                            .unwrap_or(0);
                        break (end + 4, length);
                    }
                };
                while bytes.len() < end + length {
                    let n = socket.read(&mut chunk).await.unwrap();
                    if n == 0 {
                        return;
                    }
                    bytes.extend_from_slice(&chunk[..n]);
                }

                let req_body_str = String::from_utf8_lossy(&bytes[end..end + length]);
                let is_subagent = req_body_str.contains("Task from parent agent");

                let (action, args) = if is_subagent {
                    (
                        "finish",
                        json!({"summary": "Subagent findings: all good", "verification": "verified"}),
                    )
                } else if step == 0 {
                    (
                        "dispatch_agents",
                        json!({
                            "tasks": [
                                {"role": "recon", "instruction": "survey files in swarm"},
                                {"role": "tui-tracer", "instruction": "trace tui swarm"},
                                {"role": "gui-tracer", "instruction": "trace gui strip"}
                            ]
                        }),
                    )
                } else {
                    (
                        "finish",
                        json!({"summary": "All 3 subagents finished successfully", "verification": "done"}),
                    )
                };

                let body = format!(
                    "{}\n",
                    json!({
                        "model": "qwen2.5",
                        "message": {
                            "role": "assistant",
                            "content": "",
                            "tool_calls": [{"function": {"name": action, "arguments": args}}]
                        },
                        "done": true
                    })
                );
                let _ = socket.write_all(format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: application/x-ndjson\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                ).as_bytes()).await;
            }
        });

        let chat = format!("parent-agent-turn-{}", uuid::Uuid::new_v4());
        let agent_task = tokio::spawn(async move {
            orchestrate_agent_loop(
                &cfg,
                "dispatch 3 subagents",
                &workspace,
                None,
                None,
                None,
                Some(&chat),
                None,
                None,
                None,
                true,
                false,
                |_| Ok(ApprovalOutcome::Approved),
                |_| {},
                |_| {},
            )
            .await
        });

        let res = tokio::time::timeout(std::time::Duration::from_secs(10), agent_task).await;
        let agent_res = res.unwrap().unwrap().unwrap();
        assert!(agent_res.summary.contains("All 3 subagents finished successfully") || agent_res.summary.contains("Subagent"));
        provider.abort();
    });
}

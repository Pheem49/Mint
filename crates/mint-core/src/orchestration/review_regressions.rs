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

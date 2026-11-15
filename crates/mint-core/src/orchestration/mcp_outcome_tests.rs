use super::*;
use std::time::Duration;
fn generic_server(tool: &str, response: Value) -> String {
    format!(
        r#"
import sys,json
response=json.loads({:?})
for line in sys.stdin:
 r=json.loads(line);m=r.get('method')
 if m=='initialize': result={{}}
 elif m=='tools/list': result={{'tools':[{{'name':{:?},'inputSchema':{{'type':'object'}}}}]}}
 elif m=='tools/call': result=response
 else: continue
 print(json.dumps({{'jsonrpc':'2.0','id':r['id'],'result':result}}),flush=True)
"#,
        response.to_string(),
        tool
    )
}

#[test]
fn compact_device_progress_keeps_acceptance_and_latest_observation() {
    let events=["accepted","running","running","verified"].into_iter().enumerate().map(|(seq,state)|serde_json::from_value(serde_json::json!({"type":"DeviceState","data":{"callId":"one","server":"motor","tool":"move","update":{"state":state,"target":{"rpm":100},"actual":{"rpm":seq},"observationSeq":seq,"elapsedSecs":seq,"message":"observed"}}})).unwrap()).collect::<Vec<AgentProgress>>();
    let compacted = compact_agent_progress(&events);
    assert_eq!(
        compacted.len(),
        2,
        "saved history should retain acceptance and the final fresh observation"
    );
    assert!(matches!(
        &compacted[1],
        AgentProgress::DeviceState {
            update: crate::mcp::device::DeviceUpdate {
                state: crate::mcp::device::DevicePhase::Verified,
                ..
            },
            ..
        }
    ));
}

#[tokio::test]
async fn mcp_definitions_reach_native_and_json_models_before_any_command() {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    for native in [true, false] {
        let marker =
            std::env::temp_dir().join(format!("mint-model-command-{}", uuid::Uuid::new_v4()));
        let server = format!("model-mcp-{}", uuid::Uuid::new_v4());
        let mut cfg = MintConfig::default();
        let script=generic_server("move",serde_json::json!({"content":[{"type":"text","text":"moved"}]})).replace("elif m=='tools/call': result=response",&format!("elif m=='tools/call':\n  open({:?},'a').write('called\\n')\n  result=response",marker.to_string_lossy())).replace("'name':\"move\",'inputSchema'","'name':\"move\",'description':'Move the mock axis. RPM are integer units.', 'inputSchema'");
        cfg.extra.insert("mcpServers".into(),serde_json::json!({server.clone():{"command":"python3","args":["-u","-c",script],"timeoutSecs":5}}));
        cfg.extra.insert(
            "allowedMcpTools".into(),
            serde_json::json!({server.clone():["*"]}),
        );
        cfg.ai_provider = "ollama".into();
        cfg.ollama_model = "qwen2.5".into();
        cfg.extra
            .insert("forceJsonPromptMode".into(), serde_json::json!(!native));
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        cfg.ollama_host = format!("http://{}", listener.local_addr().unwrap());
        let recorded = Arc::new(std::sync::Mutex::new(Vec::<Value>::new()));
        let requests = Arc::clone(&recorded);
        let observed_marker = marker.clone();
        let name = server.clone();
        let provider = tokio::spawn(async move {
            for step in 0..3 {
                let (mut socket, _) = listener.accept().await.unwrap();
                let mut bytes = Vec::new();
                let mut chunk = [0u8; 4096];
                let (end, len) = loop {
                    let n = socket.read(&mut chunk).await.unwrap();
                    assert!(n > 0);
                    bytes.extend_from_slice(&chunk[..n]);
                    if let Some(end) = bytes.windows(4).position(|w| w == b"\r\n\r\n") {
                        let header = String::from_utf8_lossy(&bytes[..end]);
                        let len = header
                            .lines()
                            .find_map(|l| {
                                l.to_ascii_lowercase()
                                    .strip_prefix("content-length:")
                                    .and_then(|v| v.trim().parse::<usize>().ok())
                            })
                            .unwrap();
                        break (end + 4, len);
                    }
                };
                while bytes.len() < end + len {
                    let n = socket.read(&mut chunk).await.unwrap();
                    assert!(n > 0);
                    bytes.extend_from_slice(&chunk[..n]);
                }
                requests
                    .lock()
                    .unwrap()
                    .push(serde_json::from_slice(&bytes[end..end + len]).unwrap());
                if step == 1 {
                    assert!(
                        !observed_marker.exists(),
                        "initial preparation sent a tools/call"
                    );
                }
                let (action, input) = if step < 2 {
                    (
                        "mcp_tool",
                        serde_json::json!({"server":name,"tool":"move","arguments":{}}),
                    )
                } else {
                    (
                        "finish",
                        serde_json::json!({"summary":"done","verification":"Mock command verified"}),
                    )
                };
                let message = if native {
                    serde_json::json!({"role":"assistant","content":"","tool_calls":[{"function":{"name":action,"arguments":input}}]})
                } else {
                    serde_json::json!({"role":"assistant","content":serde_json::json!({"thought":"test","action":action,"input":input}).to_string()})
                };
                let body = format!(
                    "{}\n",
                    serde_json::json!({"model":"qwen2.5","message":message,"done":true,"prompt_eval_count":100,"eval_count":10})
                );
                socket.write_all(format!("HTTP/1.1 200 OK\r\nContent-Type: application/x-ndjson\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",body.len()).as_bytes()).await.unwrap();
            }
        });
        let chat = format!("model-review-{}", uuid::Uuid::new_v4());
        let mut events = Vec::new();
        let outcome = tokio::time::timeout(
            Duration::from_secs(15),
            orchestrate_agent_loop(
                &cfg,
                "Use the mock MCP move tool",
                Path::new("."),
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
                |e| events.push(e),
                |_| {},
            ),
        )
        .await;
        provider.abort();
        crate::mcp::close_mcp_session(&server);
        assert!(
            matches!(outcome, Ok(Ok(_))),
            "native={native}, {outcome:?}; requests={}, events={:?}",
            recorded.lock().unwrap().len(),
            events.iter().rev().take(5).collect::<Vec<_>>()
        );
        let requests = recorded.lock().unwrap();
        assert_eq!(requests.len(), 3);
        assert!(
            requests[1].to_string().contains("Move the mock axis"),
            "full tool definition missing from model request: {}",
            requests[1]
        );
        assert!(
            requests[2].to_string().contains("Move the mock axis"),
            "definition must remain in later requests"
        );
        assert_eq!(std::fs::read_to_string(&marker).unwrap().lines().count(), 1);
        std::fs::remove_file(marker).unwrap();
        assert_eq!(
            events
                .iter()
                .filter(|e| matches!(e,AgentProgress::ToolStart {action,..} if action=="mcp_tool"))
                .count(),
            1
        );
        let memory = MemoryStore::open_default().unwrap();
        let _ = memory.delete_chat_session(&chat);
    }
}

#[tokio::test]
async fn device_command_waits_for_a_fresh_verified_observation() {
    let server = format!("device-{}", uuid::Uuid::new_v4());
    let mut cfg = MintConfig::default();
    cfg.extra.insert("mcpServers".into(),serde_json::json!({server.clone():{"command":"node","args":[Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/device-mcp/server.mjs")],"timeoutSecs":5}}));
    cfg.extra.insert(
        "allowedMcpTools".into(),
        serde_json::json!({server.clone():["*"]}),
    );
    let decision:AgentDecision=serde_json::from_value(serde_json::json!({"action":"mcp_tool","input":{"server":server,"tool":"set_speed","arguments":{"deviceId":"mock-device","rpm":100}}})).unwrap();
    let mut events = Vec::new();
    let out = execute_tool_outcome(
        Path::new("."),
        &cfg,
        &decision,
        "device-test",
        &mut |_| Ok(ApprovalOutcome::Denied),
        &mut |e| events.push(e),
        "device-command",
    )
    .await
    .unwrap();
    crate::mcp::close_mcp_session(&server);
    assert_eq!(
        out.status,
        crate::integrations::mcp_result::ToolStatus::Success
    );
    let events = serde_json::to_value(events).unwrap();
    assert!(
        events
            .as_array()
            .unwrap()
            .iter()
            .any(|e| e["type"] == "DeviceState" && e["data"]["update"]["state"] == "verified"),
        "receipt must not be considered a verified device state: {events}"
    );
}

#[tokio::test]
async fn device_capture_preserves_verified_images_and_explicit_stop_confirms_zero() {
    let server = format!("device-photo-{}", uuid::Uuid::new_v4());
    let mut cfg = MintConfig::default();
    cfg.extra.insert("mcpServers".into(),serde_json::json!({server.clone():{"command":"node","args":[Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/device-mcp/server.mjs")],"timeoutSecs":10}}));
    cfg.extra.insert(
        "allowedMcpTools".into(),
        serde_json::json!({server.clone():["*"]}),
    );
    let photo:AgentDecision=serde_json::from_value(serde_json::json!({"action":"mcp_tool","input":{"server":server,"tool":"capture_photo","arguments":{"deviceId":"mock-device"}}})).unwrap();
    let out = execute_tool_outcome(
        Path::new("."),
        &cfg,
        &photo,
        "photo-test",
        &mut |_| Ok(ApprovalOutcome::Denied),
        &mut |_| {},
        "photo-command",
    )
    .await
    .unwrap();
    assert_eq!(
        out.status,
        crate::integrations::mcp_result::ToolStatus::Success
    );
    assert_eq!(out.images.len(), 1);
    assert!(out.warnings.is_empty());
    remove_test_artifacts(&out.artifacts);
    crate::mcp::call_mcp_tool_async(
        &cfg,
        &server,
        "set_speed",
        serde_json::json!({"deviceId":"mock-device","rpm":1000}),
        "photo-test",
        |_| {},
    )
    .await
    .unwrap();
    let stop:AgentDecision=serde_json::from_value(serde_json::json!({"action":"mcp_tool","input":{"server":server,"tool":"stop","arguments":{"deviceId":"mock-device"}}})).unwrap();
    let mut events = Vec::new();
    let out = execute_tool_outcome(
        Path::new("."),
        &cfg,
        &stop,
        "photo-test",
        &mut |_| Ok(ApprovalOutcome::Denied),
        &mut |e| events.push(e),
        "stop-command",
    )
    .await
    .unwrap();
    crate::mcp::close_mcp_session(&server);
    assert_eq!(
        out.status,
        crate::integrations::mcp_result::ToolStatus::Success
    );
    assert!(events.iter().any(|e|matches!(e,AgentProgress::DeviceState {update,..} if matches!(update.state,crate::mcp::device::DevicePhase::Verified) && update.actual["rpm"]==0)));
}

fn device_fixture(status: Value, timeout: u64) -> (MintConfig, String, PathBuf) {
    let name = format!("device-fault-{}", uuid::Uuid::new_v4());
    let log = std::env::temp_dir().join(format!("mint-device-calls-{}", uuid::Uuid::new_v4()));
    let schema = serde_json::json!({"type":"object","properties":{"deviceId":{"type":"string"},"operationId":{"type":"string"},"rpm":{"type":"integer"}},"required":["deviceId"],"additionalProperties":false});
    let tools = serde_json::json!([{"name":"set_speed","inputSchema":schema,"outputSchema":{"type":"object"},"_meta":{"mint/device":{"version":1,"statusTool":"get_status"}}},{"name":"get_status","inputSchema":schema,"outputSchema":{"type":"object"}}]);
    let receipt = serde_json::json!({"deviceId":"mock","operationId":"one","state":"accepted","observationSeq":1,"target":{"rpm":100},"actual":{"rpm":0}});
    let script = format!(
        r#"
import sys,json
tools=json.loads({:?});receipt=json.loads({:?});status=json.loads({:?})
for line in sys.stdin:
 r=json.loads(line);m=r.get('method')
 if m=='initialize': result={{}}
 elif m=='tools/list': result={{'tools':tools}}
 elif m=='tools/call':
  with open({:?},'a') as f: f.write(json.dumps(r['params'])+'\n')
  result={{'structuredContent':receipt if r['params']['name']=='set_speed' else status,'content':[]}}
 else: continue
 print(json.dumps({{'jsonrpc':'2.0','id':r['id'],'result':result}}),flush=True)
"#,
        tools.to_string(),
        receipt.to_string(),
        status.to_string(),
        log.to_string_lossy()
    );
    let mut cfg = MintConfig::default();
    cfg.extra.insert("mcpServers".into(),serde_json::json!({name.clone():{"command":"python3","args":["-u","-c",script],"timeoutSecs":timeout}}));
    cfg.extra.insert(
        "allowedMcpTools".into(),
        serde_json::json!({name.clone():["*"]}),
    );
    (cfg, name, log)
}

#[tokio::test]
async fn device_stale_observations_cannot_confirm_completion() {
    let (cfg, server, log) = device_fixture(
        serde_json::json!({"deviceId":"mock","operationId":"one","state":"completed","observationSeq":1,"target":{"rpm":100},"actual":{"rpm":100}}),
        1,
    );
    let decision:AgentDecision=serde_json::from_value(serde_json::json!({"action":"mcp_tool","input":{"server":server,"tool":"set_speed","arguments":{"deviceId":"mock","rpm":100}}})).unwrap();
    let out = execute_tool_outcome(
        Path::new("."),
        &cfg,
        &decision,
        "stale-test",
        &mut |_| Ok(ApprovalOutcome::Denied),
        &mut |_| {},
        "stale-command",
    )
    .await
    .unwrap();
    crate::mcp::close_mcp_session(&server);
    let calls = std::fs::read_to_string(&log).unwrap();
    std::fs::remove_file(log).unwrap();
    assert_eq!(calls.lines().filter(|l| l.contains("set_speed")).count(), 1);
    assert_eq!(
        out.status,
        crate::integrations::mcp_result::ToolStatus::TimedOut
    );
    assert!(
        out.completion_unconfirmed,
        "a dispatched command remains uncertain after stale observations"
    );
}

#[tokio::test]
async fn review_device_session_loss_preserves_receipt_without_reconnecting_or_repeating() {
    let (mut cfg, server, log) = device_fixture(
        serde_json::json!({"deviceId":"mock","operationId":"one","state":"running","observationSeq":2,"target":{"rpm":100},"actual":{"rpm":50}}),
        5,
    );
    let starts = log.with_extension("starts");
    let script = cfg.extra["mcpServers"][&server]["args"][2]
        .as_str()
        .unwrap()
        .replace(
            "import sys,json",
            &format!(
                "import sys,json,os\nopen({:?},'a').write('start\\n')",
                starts.to_string_lossy()
            ),
        );
    cfg.extra.get_mut("mcpServers").unwrap()[&server]["args"][2] = serde_json::json!(format!(
        "{script}\n if m=='tools/call' and r['params']['name']=='get_status': os._exit(0)\n"
    ));
    let decision=serde_json::from_value(serde_json::json!({"action":"mcp_tool","input":{"server":server,"tool":"set_speed","arguments":{"deviceId":"mock","rpm":100}}})).unwrap();
    let mut events = Vec::new();
    let out = execute_tool_outcome(
        Path::new("."),
        &cfg,
        &decision,
        "review-device-loss",
        &mut |_| Ok(ApprovalOutcome::Denied),
        &mut |e| events.push(e),
        "command",
    )
    .await
    .unwrap();
    crate::mcp::close_mcp_session(&server);
    let calls = std::fs::read_to_string(&log).unwrap();
    assert_eq!(std::fs::read_to_string(&starts).unwrap(), "start\n");
    let _ = std::fs::remove_file(log);
    let _ = std::fs::remove_file(starts);
    assert_eq!(calls.lines().filter(|l| l.contains("set_speed")).count(), 1);
    assert_eq!(
        calls.lines().filter(|l| l.contains("get_status")).count(),
        1
    );
    assert_eq!(out.status, crate::mcp_result::ToolStatus::Failed);
    assert!(out.completion_unconfirmed);
    assert!(
        out.text.contains("one"),
        "operation receipt lost: {}",
        out.text
    );
    assert!(events.iter().any(|e|matches!(e,AgentProgress::DeviceState{update,..} if matches!(update.state,crate::mcp::device::DevicePhase::Unconfirmed) && update.operation_id.as_deref()==Some("one"))));
    assert!(!events.iter().any(|e|matches!(e,AgentProgress::DeviceState{update,..} if matches!(update.state,crate::mcp::device::DevicePhase::Verified))));
}

#[tokio::test]
async fn device_wrong_operation_and_physical_failure_never_report_success() {
    for status in [
        serde_json::json!({"deviceId":"mock","operationId":"other","state":"completed","observationSeq":2,"target":{"rpm":100},"actual":{"rpm":100}}),
        serde_json::json!({"deviceId":"mock","operationId":"one","state":"failed","observationSeq":2,"target":{"rpm":100},"actual":{"rpm":0},"error":"jammed"}),
    ] {
        let (cfg, server, log) = device_fixture(status, 3);
        let decision:AgentDecision=serde_json::from_value(serde_json::json!({"action":"mcp_tool","input":{"server":server,"tool":"set_speed","arguments":{"deviceId":"mock","rpm":100}}})).unwrap();
        let out = execute_tool_outcome(
            Path::new("."),
            &cfg,
            &decision,
            "wrong-operation",
            &mut |_| Ok(ApprovalOutcome::Denied),
            &mut |_| {},
            "wrong-command",
        )
        .await
        .unwrap();
        crate::mcp::close_mcp_session(&server);
        std::fs::remove_file(log).unwrap();
        assert_eq!(
            out.status,
            crate::integrations::mcp_result::ToolStatus::Failed
        );
        assert!(
            out.text.starts_with("Error:"),
            "Native tool results must carry an error marker for failed verification: {}",
            out.text
        );
        assert!(
            !out.text.contains("no tool was executed"),
            "verification errors must acknowledge the already dispatched command: {}",
            out.text
        );
    }
}

#[tokio::test]
async fn device_failed_receipt_is_terminal_without_status_polling() {
    let (mut cfg, server, log) = device_fixture(
        serde_json::json!({"deviceId":"mock","operationId":"one","state":"completed","observationSeq":2,"target":{"rpm":100},"actual":{"rpm":100}}),
        3,
    );
    let script = cfg.extra["mcpServers"][&server]["args"][2]
        .as_str()
        .unwrap()
        .replace("accepted", "failed");
    cfg.extra.get_mut("mcpServers").unwrap()[&server]["args"][2] = serde_json::json!(script);
    let decision:AgentDecision=serde_json::from_value(serde_json::json!({"action":"mcp_tool","input":{"server":server,"tool":"set_speed","arguments":{"deviceId":"mock","rpm":100}}})).unwrap();
    let out = execute_tool_outcome(
        Path::new("."),
        &cfg,
        &decision,
        "failed-receipt",
        &mut |_| Ok(ApprovalOutcome::Denied),
        &mut |_| {},
        "failed-command",
    )
    .await
    .unwrap();
    crate::mcp::close_mcp_session(&server);
    let calls = std::fs::read_to_string(&log).unwrap();
    std::fs::remove_file(log).unwrap();
    assert_eq!(
        out.status,
        crate::integrations::mcp_result::ToolStatus::Failed
    );
    assert!(
        !calls.contains("get_status"),
        "failed receipt must not be overwritten by a later status"
    );
}

#[tokio::test]
async fn device_denied_status_access_blocks_movement_before_dispatch() {
    let (mut cfg, server, log) = device_fixture(serde_json::json!({}), 3);
    cfg.extra.insert(
        "allowedMcpTools".into(),
        serde_json::json!({server.clone():["set_speed"]}),
    );
    let decision:AgentDecision=serde_json::from_value(serde_json::json!({"action":"mcp_tool","input":{"server":server,"tool":"set_speed","arguments":{"deviceId":"mock","rpm":100}}})).unwrap();
    let mut approvals = Vec::new();
    let out = execute_tool_outcome(
        Path::new("."),
        &cfg,
        &decision,
        "denied-status",
        &mut |a| {
            approvals.push(serde_json::to_value(a).unwrap());
            Ok(ApprovalOutcome::Denied)
        },
        &mut |_| {},
        "blocked-command",
    )
    .await
    .unwrap();
    crate::mcp::close_mcp_session(&server);
    assert!(
        !log.exists(),
        "command was dispatched without access to status verification"
    );
    assert_eq!(
        out.status,
        crate::integrations::mcp_result::ToolStatus::Failed
    );
    assert!(
        serde_json::to_string(&approvals)
            .unwrap()
            .contains("get_status")
    );
}

#[tokio::test]
async fn canceling_between_device_observations_does_not_issue_another_command() {
    let (cfg, server, log) = device_fixture(
        serde_json::json!({"deviceId":"mock","operationId":"one","state":"running","observationSeq":2,"target":{"rpm":100},"actual":{"rpm":20}}),
        15,
    );
    let decision:AgentDecision=serde_json::from_value(serde_json::json!({"action":"mcp_tool","input":{"server":server,"tool":"set_speed","arguments":{"deviceId":"mock","rpm":100}}})).unwrap();
    let chat = format!("cancel-between-{}", uuid::Uuid::new_v4());
    let name = chat.clone();
    let (tx, rx) = tokio::sync::oneshot::channel();
    let mut tx = Some(tx);
    let running = tokio::spawn(async move {
        execute_tool_outcome(
            Path::new("."),
            &cfg,
            &decision,
            &name,
            &mut |_| Ok(ApprovalOutcome::Denied),
            &mut |e| {
                if matches!(
                    e,
                    AgentProgress::DeviceState {
                        update: crate::mcp::device::DeviceUpdate {
                            state: crate::mcp::device::DevicePhase::Running,
                            ..
                        },
                        ..
                    }
                ) && let Some(tx) = tx.take()
                {
                    let _ = tx.send(());
                }
            },
            "cancel-command",
        )
        .await
        .unwrap()
    });
    tokio::time::timeout(Duration::from_secs(10), rx)
        .await
        .unwrap()
        .unwrap();
    tokio::time::sleep(Duration::from_millis(100)).await;
    assert!(crate::mcp::cancel_mcp_calls(&chat) > 0);
    let out = tokio::time::timeout(Duration::from_secs(2), running)
        .await
        .unwrap()
        .unwrap();
    crate::mcp::close_mcp_session(&server);
    assert_eq!(
        out.status,
        crate::integrations::mcp_result::ToolStatus::Cancelled
    );
    let calls = std::fs::read_to_string(&log).unwrap();
    std::fs::remove_file(log).unwrap();
    assert_eq!(calls.lines().filter(|l| l.contains("set_speed")).count(), 1);
    assert!(!calls.contains("\"stop\""));
}

#[tokio::test]
async fn approval_feedback_does_not_mark_an_unexecuted_mcp_call_successful() {
    let decision: AgentDecision = serde_json::from_value(serde_json::json!({
        "action":"mcp_tool", "input":{"server":"feedback-server","tool":"render","arguments":{}}
    }))
    .unwrap();
    let mut config = MintConfig::default();
    config.extra.insert("mcpServers".into(),serde_json::json!({"feedback-server":{"command":"python3","args":["-u","-c",generic_server("render",serde_json::json!({}))]}}));
    let out = execute_tool_outcome(
        Path::new("."),
        &config,
        &decision,
        "feedback",
        &mut |_| {
            Ok(ApprovalOutcome::Intercepted(
                "Use a different device".into(),
            ))
        },
        &mut |_| {},
        "not-executed",
    )
    .await
    .unwrap();
    assert!(out.text.contains("Use a different device"));
    assert!(out.text.contains("no command was executed"));
    assert_eq!(
        out.status,
        crate::integrations::mcp_result::ToolStatus::Failed
    );
}

#[tokio::test]
async fn text_only_model_keeps_artifacts_without_receiving_image_blocks() {
    use base64::Engine as _;
    let mut bytes = std::io::Cursor::new(Vec::new());
    image::DynamicImage::new_rgb8(1, 1)
        .write_to(&mut bytes, image::ImageFormat::Png)
        .unwrap();
    let encoded = base64::engine::general_purpose::STANDARD.encode(bytes.into_inner());
    let response =
        serde_json::json!({"content":[{"type":"image","mimeType":"image/png","data":encoded}]})
            .to_string();
    let script = generic_server("preview", serde_json::from_str(&response).unwrap());
    let server = format!("text-model-{}", uuid::Uuid::new_v4());
    let mut cfg = MintConfig::default();
    cfg.extra.insert(
        "mcpServers".into(),
        serde_json::json!({server.clone():{"command":"python3","args":["-u","-c",script]}}),
    );
    cfg.extra.insert(
        "allowedMcpTools".into(),
        serde_json::json!({server.clone():["preview"]}),
    );
    cfg.extra.insert("mcpImageInput".into(), Value::Bool(false));
    let decision:AgentDecision=serde_json::from_value(serde_json::json!({"action":"mcp_tool","input":{"server":server,"tool":"preview","arguments":{}}})).unwrap();
    let out = execute_tool_outcome(
        Path::new("."),
        &cfg,
        &decision,
        "text-only",
        &mut |_| Ok(ApprovalOutcome::Denied),
        &mut |_| {},
        "preview",
    )
    .await
    .unwrap();
    assert!(out.images.is_empty());
    assert_eq!(out.artifacts.len(), 1);
    assert!(out.text.contains("do not claim visual inspection"));
    crate::mcp::close_mcp_session(&server);
    remove_test_artifacts(&out.artifacts);
}

#[test]
fn multiple_images_attach_to_the_correct_native_tool_result() {
    let images = std::collections::HashMap::from([(
        "call-2".to_string(),
        vec![
            "data:image/png;base64,first".into(),
            "data:image/png;base64,second".into(),
        ],
    )]);
    let results = vec![
        (
            "call-1".into(),
            "mcp_tool".into(),
            Value::Null,
            "one".into(),
        )
            .into(),
        (
            "call-2".into(),
            "mcp_tool".into(),
            Value::Null,
            "two".into(),
        )
            .into(),
    ];
    let mut messages = Vec::new();
    append_native_tool_results(&mut messages, "", &results, &Default::default(), &images);
    assert!(
        matches!(&messages[1].content[1],ContentBlock::ToolResult {tool_use_id,..} if tool_use_id=="call-2")
    );
    assert!(
        matches!(&messages[1].content[2],ContentBlock::Image {data_uri} if data_uri.ends_with("first"))
    );
    assert!(
        matches!(&messages[1].content[3],ContentBlock::Image {data_uri} if data_uri.ends_with("second"))
    );
}

#[tokio::test]
async fn mcp_images_do_not_enter_the_text_observation() {
    let server = format!("image-{}", uuid::Uuid::new_v4());
    let mut bytes = std::io::Cursor::new(Vec::new());
    image::DynamicImage::new_rgb8(1, 1)
        .write_to(&mut bytes, image::ImageFormat::Png)
        .unwrap();
    let png = BASE64_STANDARD.encode(bytes.into_inner());
    let result = serde_json::json!({"content":[{"type":"text","text":"preview"},{"type":"image","mimeType":"image/png","data":png}]}).to_string();
    let script = generic_server("preview", serde_json::from_str(&result).unwrap());
    let mut config = MintConfig::default();
    config.extra.insert(
        "mcpServers".into(),
        serde_json::json!({server.clone():{"command":"python3","args":["-u","-c",script]}}),
    );
    let decision: AgentDecision = serde_json::from_value(serde_json::json!({"action":"mcp_tool","input":{"server":server,"tool":"preview","arguments":{}}})).unwrap();
    let mut events = Vec::new();
    let out = execute_tool_outcome(
        Path::new("."),
        &config,
        &decision,
        "image-test",
        &mut |_| Ok(ApprovalOutcome::Approved),
        &mut |event| events.push(event),
        "image-invocation",
    )
    .await
    .unwrap();
    assert!(
        !out.text.contains(&png),
        "binary data must be removed from the model's text observation"
    );
    assert_eq!(
        out.status,
        crate::integrations::mcp_result::ToolStatus::Success
    );
    assert_eq!(out.images.len(), 1);
    assert_eq!(out.artifacts[0].call_id, "image-invocation");
    assert!(out.text.contains("preview"));
    assert!(!serde_json::to_string(&events).unwrap().contains(&png));
    assert!(
        !crate::mcp::is_mcp_tool_allowed(&config, &server, "preview"),
        "one-call approval must not persist a grant"
    );
    crate::mcp::close_mcp_session(&server);
    remove_test_artifacts(&out.artifacts);
}

fn remove_test_artifacts(artifacts: &[crate::integrations::mcp_result::McpArtifact]) {
    let directory = crate::integrations::mcp_result::artifact_directory().unwrap();
    for artifact in artifacts {
        std::fs::remove_file(directory.join(&artifact.id)).unwrap();
        std::fs::remove_file(directory.join(format!("{}.json", artifact.id))).unwrap();
    }
}

// A transport-successful tool failure must not become a successful agent step.
#[tokio::test]
async fn mcp_tool_failure_is_visible_to_the_agent() {
    let server = format!("outcome-{}", uuid::Uuid::new_v4());
    let mut config = MintConfig::default();
    config.extra.insert("mcpServers".into(),serde_json::json!({server.clone():{"command":"python3","args":["-u","-c",generic_server("render",serde_json::json!({"isError":true,"content":[{"type":"text","text":"render failed"}]}))]}}));
    config.extra.insert(
        "allowedMcpTools".into(),
        serde_json::json!({server.clone():["render"]}),
    );
    let decision: AgentDecision = serde_json::from_value(serde_json::json!({
            "thought":"render", "action":"mcp_tool", "input":{"server":server,"tool":"render","arguments":{}}
        })).unwrap();
    let result = execute_tool(
        Path::new("."),
        &config,
        &decision,
        "outcome-test",
        &mut |_| Ok(ApprovalOutcome::Denied),
        &mut |_| {},
    )
    .await
    .unwrap();
    assert!(
        result.starts_with("Error:"),
        "tool failure must reach the model as a failure: {result}"
    );
    assert!(result.contains("render failed"));
    crate::mcp::close_mcp_session(&server);
}

#[test]
fn test_fault_tolerant_null_handling() {
    let input = serde_json::json!({
        "command": "curl -sL https://example.com",
        "city": null,
        "columns": null,
        "verification": null,
        "filter": null
    });
    let parsed = decode_agent_input(input).unwrap();
    assert_eq!(parsed.command, "curl -sL https://example.com");
    assert_eq!(parsed.city, "");
    assert_eq!(parsed.verification, "");
}

#[test]
fn test_unpack_nested_arguments() {
    // Stringified JSON arguments
    let str_args = serde_json::json!({
        "action": "run_shell",
        "arguments": "{\"command\": \"ls -la\", \"city\": null}"
    });
    let parsed_str = decode_agent_input(str_args).unwrap();
    assert_eq!(parsed_str.command, "ls -la");

    // Object arguments
    let obj_args = serde_json::json!({
        "action": "run_shell",
        "arguments": { "command": "pwd" }
    });
    let parsed_obj = decode_agent_input(obj_args).unwrap();
    assert_eq!(parsed_obj.command, "pwd");
}

#[test]
fn test_shell_command_aliases() {
    for (alias, expected) in [
        ("cmd", "echo 1"),
        ("script", "echo 2"),
        ("exec", "echo 3"),
        ("code", "echo 4"),
        ("bash", "echo 5"),
        ("sh", "echo 6"),
    ] {
        let input = serde_json::json!({ alias: expected });
        let parsed = decode_agent_input(input).unwrap();
        assert_eq!(parsed.command, expected, "Failed for alias {alias}");
    }
}

#[test]
fn test_parse_tool_input_raw_shell_string() {
    let raw = Value::String("curl -sL --max-time 25 https://sndiary02.com".into());
    let parsed = parse_tool_input("run_shell", raw).unwrap();
    assert_eq!(
        parsed.command,
        "curl -sL --max-time 25 https://sndiary02.com"
    );
}

#[test]
fn test_parse_tool_input_prunes_dummy_null_strings() {
    let raw = serde_json::json!({
        "command": "curl -sL https://example.com",
        "path": "null",
        "query": "null",
        "name": "null",
        "title": "undefined",
        "url": "none"
    });
    let parsed = parse_tool_input("run_shell", raw).unwrap();
    assert_eq!(parsed.command, "curl -sL https://example.com");
    assert_eq!(parsed.path, "");
    assert_eq!(parsed.query, "");
    assert_eq!(parsed.name, "");
    assert_eq!(parsed.title, "");
    assert_eq!(parsed.url, "");
}

#[test]
fn test_is_dummy_value() {
    assert!(is_dummy_value("null"));
    assert!(is_dummy_value("NULL"));
    assert!(is_dummy_value("  null  "));
    assert!(is_dummy_value("undefined"));
    assert!(is_dummy_value("none"));
    assert!(is_dummy_value("nil"));
    assert!(is_dummy_value("(null)"));
    assert!(is_dummy_value("\"\""));
    assert!(is_dummy_value("''"));

    assert!(!is_dummy_value("echo null"));
    assert!(!is_dummy_value("cat /dev/null"));
    assert!(!is_dummy_value("src/main.rs"));
    assert!(!is_dummy_value("pwd"));
}

#![cfg(unix)]
use mint_core::{MintConfig, mcp};
use serde_json::{Value, json};

#[test]
fn review_reconnect_cannot_dispatch_using_the_previous_sessions_schema() {
    let state = std::env::temp_dir().join(format!("mint-review-restart-{}", uuid::Uuid::new_v4()));
    let log = state.with_extension("calls");
    let name = format!("review-restart-{}", uuid::Uuid::new_v4());
    let script = format!(
        r#"
import sys,json,os,pathlib
state=pathlib.Path({:?});version=int(state.read_text())+1 if state.exists() else 1;state.write_text(str(version))
for line in sys.stdin:
 r=json.loads(line);m=r.get('method')
 if m=='initialize': result={{}}
 elif m=='tools/list': result={{'tools':[{{'name':'move','description':'x'*240000,'inputSchema':{{'type':'object','properties':{{'rpm':{{'type':'integer','maximum':100 if version==1 else 0}}}},'required':['rpm']}}}}]}}
 elif m=='tools/call':
  open({:?},'a').write(str(version)+'\n')
  result={{'executed':version}}
 else: continue
 print(json.dumps({{'jsonrpc':'2.0','id':r['id'],'result':result}}),flush=True)
 if m=='tools/list' and version==1: os._exit(0)
"#,
        state.to_string_lossy(),
        log.to_string_lossy()
    );
    let mut cfg = MintConfig::default();
    cfg.extra.insert(
        "mcpServers".into(),
        json!({name.clone():{"command":"python3","args":["-u","-c",script],"timeoutSecs":3}}),
    );
    cfg.extra
        .insert("allowedMcpTools".into(), json!({name.clone():["*"]}));
    let result = mcp::call_mcp_tool(&cfg, &name, "move", json!({"rpm":50}));
    let sent = std::fs::read_to_string(&log).unwrap_or_default();
    assert!(
        sent.is_empty(),
        "a replacement server received an unvalidated command: {sent}"
    );
    assert!(
        result.is_err(),
        "session loss must reject dispatch: {result:?}"
    );
    // A later explicit invocation may reconnect, but must load the new schema.
    assert!(mcp::call_mcp_tool(&cfg, &name, "move", json!({"rpm":50})).is_err());
    assert_eq!(
        mcp::call_mcp_tool(&cfg, &name, "move", json!({"rpm":0})).unwrap()["executed"],
        2
    );
    mcp::close_mcp_session(&name);
    assert_eq!(std::fs::read_to_string(&log).unwrap(), "2\n");
    let _ = std::fs::remove_file(state);
    let _ = std::fs::remove_file(log);
}

#[test]
fn review_pagination_session_loss_cannot_mix_catalogs_or_start_a_replacement() {
    let state = std::env::temp_dir().join(format!("mint-page-session-{}", uuid::Uuid::new_v4()));
    let script = format!(
        r#"
import sys,json,os,pathlib
state=pathlib.Path({:?});version=int(state.read_text())+1 if state.exists() else 1;state.write_text(str(version))
for line in sys.stdin:
 r=json.loads(line);m=r.get('method')
 if m=='initialize': result={{}}
 elif m=='tools/list':
  result={{'tools':[{{'name':'first' if version==1 else 'last','description':'x'*240000,'inputSchema':{{'type':'object'}}}}]}}
  if version==1: result['nextCursor']='next'
 else: continue
 print(json.dumps({{'jsonrpc':'2.0','id':r['id'],'result':result}}),flush=True)
 if m=='tools/list' and version==1: os._exit(0)
"#,
        state.to_string_lossy()
    );
    let name = format!("page-session-{}", uuid::Uuid::new_v4());
    let mut cfg = MintConfig::default();
    cfg.extra.insert(
        "mcpServers".into(),
        json!({name.clone():{"command":"python3","args":["-u","-c",script]}}),
    );
    let result = mcp::list_server_tools(&cfg, &name);
    mcp::close_mcp_session(&name);
    let starts = std::fs::read_to_string(&state).unwrap();
    let _ = std::fs::remove_file(state);
    assert!(
        result.is_err(),
        "mixed or partial catalog escaped: {result:?}"
    );
    assert_eq!(starts, "1", "pagination silently reconnected");
}

fn server(tools: Value, extra: &str) -> (MintConfig, String) {
    let name = format!("contract-{}", uuid::Uuid::new_v4());
    let script = format!(
        r#"
import sys,json
tools=json.loads({:?}); lists=0; calls=0
def emit(r,result): print(json.dumps({{'jsonrpc':'2.0','id':r['id'],'result':result}}),flush=True)
for line in sys.stdin:
 r=json.loads(line); method=r.get('method')
 if method=='initialize': emit(r,{{}})
 elif method=='tools/list':
  lists+=1
  emit(r,{{'tools':tools}})
 elif method=='tools/call':
  calls+=1
  {}
  emit(r,{{'arguments':r['params']['arguments'],'lists':lists,'calls':calls}})
"#,
        tools.to_string(),
        extra
    );
    let mut cfg = MintConfig::default();
    cfg.extra.insert(
        "mcpServers".into(),
        json!({name.clone():{"command":"python3","args":["-u","-c",script],"timeoutSecs":5}}),
    );
    cfg.extra
        .insert("allowedMcpTools".into(), json!({name.clone():["*"]}));
    (cfg, name)
}

#[test]
fn schema_failures_never_send_commands_and_valid_arguments_are_unchanged() {
    let (cfg, name) = server(
        json!([{"name":"move","description":"Move motor","inputSchema":{"type":"object","$defs":{"point":{"type":"integer","minimum":0,"maximum":100}},"properties":{"position":{"$ref":"#/$defs/point"},"mode":{"enum":["absolute","relative"]}},"required":["position","mode"],"additionalProperties":false}}]),
        "pass",
    );
    for args in [
        json!({}),
        json!({"position":"2","mode":"absolute"}),
        json!({"position":101,"mode":"absolute"}),
        json!({"position":2,"mode":"guessed"}),
        json!({"position":2,"mode":"absolute","extra":1}),
    ] {
        let error = mcp::call_mcp_tool(&cfg, &name, "move", args).unwrap_err();
        assert!(error.to_string().contains("no tool was executed"));
    }
    assert!(mcp::call_mcp_tool(&cfg, &name, "invented", json!({})).is_err());
    let result =
        mcp::call_mcp_tool(&cfg, &name, "move", json!({"position":2,"mode":"absolute"})).unwrap();
    assert_eq!(result["arguments"], json!({"position":2,"mode":"absolute"}));
    assert_eq!(result["calls"], 1);
    assert_eq!(result["lists"], 1);
    mcp::close_mcp_session(&name);
}

#[test]
fn missing_and_external_schemas_are_blocked_without_reading_files_or_network() {
    let (cfg, name) = server(
        json!([{"name":"missing"},{"name":"external","inputSchema":{"type":"object","$ref":"file:///etc/passwd"}},{"name":"network","inputSchema":{"type":"object","$ref":"http://127.0.0.1:1/schema"}},{"name":"valid","inputSchema":{"type":"object"}}]),
        "pass",
    );
    for tool in ["missing", "external", "network"] {
        assert!(mcp::call_mcp_tool(&cfg, &name, tool, json!({})).is_err());
    }
    assert_eq!(
        mcp::call_mcp_tool(&cfg, &name, "valid", json!({})).unwrap()["calls"],
        1
    );
    mcp::close_mcp_session(&name);
}

#[test]
fn list_changed_invalidates_schema_before_the_next_command() {
    let (cfg, name) = server(
        json!([{"name":"move","inputSchema":{"type":"object","properties":{"rpm":{"type":"integer","maximum":100}},"required":["rpm"]}}]),
        "tools[0]['inputSchema']['properties']['rpm']['maximum']=1\n  print(json.dumps({'jsonrpc':'2.0','method':'notifications/tools/list_changed'}),flush=True)",
    );
    mcp::call_mcp_tool(&cfg, &name, "move", json!({"rpm":50})).unwrap();
    assert!(mcp::call_mcp_tool(&cfg, &name, "move", json!({"rpm":50})).is_err());
    let result = mcp::call_mcp_tool(&cfg, &name, "move", json!({"rpm":1})).unwrap();
    assert_eq!(result["lists"], 2);
    assert_eq!(result["calls"], 2);
    mcp::close_mcp_session(&name);
}

#[test]
fn discovery_walks_every_page() {
    let name = format!("pages-{}", uuid::Uuid::new_v4());
    let mut cfg = MintConfig::default();
    cfg.extra.insert(
        "mcpServers".into(),
        json!({name.clone():{"command":"python3","args":["-u","-c",r#"
import sys,json
for line in sys.stdin:
 r=json.loads(line); m=r.get('method')
 if m=='initialize': result={}
 elif m=='tools/list':
  second=r.get('params',{}).get('cursor')=='next'
  result={'tools':[{'name':'last' if second else 'first','inputSchema':{'type':'object'}}]}
  if not second: result['nextCursor']='next'
 elif m=='tools/call': result={'ok':True}
 else: continue
 print(json.dumps({'jsonrpc':'2.0','id':r['id'],'result':result}),flush=True)
"#]}}),
    );
    cfg.extra
        .insert("allowedMcpTools".into(), json!({name.clone():["*"]}));
    assert_eq!(
        mcp::call_mcp_tool(&cfg, &name, "last", json!({})).unwrap(),
        json!({"ok":true})
    );
    assert_eq!(
        mcp::list_server_tools(&cfg, &name).unwrap()["tools"]
            .as_array()
            .unwrap()
            .len(),
        2
    );
    mcp::close_mcp_session(&name);
}

#[test]
fn changed_server_configuration_starts_a_fresh_catalog() {
    let (cfg, name) = server(
        json!([{"name":"move","inputSchema":{"type":"object","properties":{"rpm":{"maximum":100}},"required":["rpm"]}}]),
        "pass",
    );
    mcp::call_mcp_tool(&cfg, &name, "move", json!({"rpm":50})).unwrap();
    let mut changed = cfg.clone();
    let script = changed.extra["mcpServers"][&name]["args"][2]
        .as_str()
        .unwrap()
        .replace("100", "1");
    changed.extra.get_mut("mcpServers").unwrap()[&name]["args"][2] = json!(script);
    assert!(mcp::call_mcp_tool(&changed, &name, "move", json!({"rpm":50})).is_err());
    assert_eq!(
        mcp::call_mcp_tool(&changed, &name, "move", json!({"rpm":1})).unwrap()["lists"],
        1
    );
    mcp::close_mcp_session(&name);
}

#[test]
fn repeated_pagination_cursor_never_exposes_a_partial_catalog() {
    let name = format!("cursor-{}", uuid::Uuid::new_v4());
    let mut cfg = MintConfig::default();
    cfg.extra.insert(
        "mcpServers".into(),
        json!({name.clone():{"command":"python3","args":["-u","-c",r#"
import sys,json
for line in sys.stdin:
 r=json.loads(line);m=r.get('method')
 if m=='initialize': result={}
 elif m=='tools/list': result={'tools':[],'nextCursor':'repeat'}
 elif m=='tools/call': result={'sent':True}
 else: continue
 print(json.dumps({'jsonrpc':'2.0','id':r['id'],'result':result}),flush=True)
"#]}}),
    );
    let error = mcp::list_server_tools(&cfg, &name).unwrap_err();
    assert!(error.to_string().contains("repeated a cursor"));
    mcp::close_mcp_session(&name);
}

#[tokio::test]
async fn discovery_deadline_covers_all_pages_in_agent_listing() {
    let name = format!("deadline-{}", uuid::Uuid::new_v4());
    let mut cfg = MintConfig::default();
    cfg.extra.insert(
        "mcpServers".into(),
        json!({name.clone():{"command":"python3","args":["-u","-c",r#"
import sys,json,time
for line in sys.stdin:
 r=json.loads(line);m=r.get('method')
 if m=='initialize': result={}
 elif m=='tools/list':
  time.sleep(16)
  result={'tools':[]}
  if not r.get('params',{}).get('cursor'): result['nextCursor']='next'
 else: continue
 print(json.dumps({'jsonrpc':'2.0','id':r['id'],'result':result}),flush=True)
"#]}}),
    );
    let result = mcp::catalog::list_tools(&cfg, &name, "catalog-deadline").await;
    mcp::close_mcp_session(&name);
    assert!(
        matches!(result, Err(mcp::McpError::Timeout)),
        "all pages must share one discovery deadline: {result:?}"
    );
}

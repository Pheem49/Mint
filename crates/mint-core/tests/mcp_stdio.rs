use mint_core::{MintConfig, call_mcp_tool, configured_mcp_servers};
use serde_json::json;

#[test]
fn reads_servers_from_config() {
    let mut config = MintConfig::default();
    config.extra.insert(
        "mcpServers".into(),
        json!({
            "echo": {
                "command": "echo",
                "args": ["ok"],
                "env": { "TOKEN": "value" }
            }
        }),
    );
    let servers = configured_mcp_servers(&config).unwrap();
    assert_eq!(servers["echo"].command, "echo");
    assert_eq!(servers["echo"].env["TOKEN"], "value");
}

#[cfg(unix)]
#[test]
fn calls_stdio_mcp_tool() {
    let mut config = MintConfig::default();
    config.extra.insert(
        "mcpServers".into(),
        json!({
            "fake": {
                "command": "python3",
                "args": ["-u","-c",r#"
import sys,json
for line in sys.stdin:
 r=json.loads(line);m=r.get('method')
 if m=='initialize': result={}
 elif m=='tools/list': result={'tools':[{'name':'ping','inputSchema':{'type':'object'}}]}
 elif m=='tools/call': result={'ok':True}
 else: continue
 print(json.dumps({'jsonrpc':'2.0','id':r['id'],'result':result}),flush=True)
"#]
            }
        }),
    );
    config
        .extra
        .insert("allowedMcpTools".into(), json!({ "fake": ["ping"] }));
    assert_eq!(
        call_mcp_tool(&config, "fake", "ping", json!({})).unwrap(),
        json!({ "ok": true })
    );
}

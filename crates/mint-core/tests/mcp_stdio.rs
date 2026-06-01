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
                "command": "sh",
                "args": [
                    "-c",
                    "read init; read ready; read call; printf '{\"jsonrpc\":\"2.0\",\"id\":2,\"result\":{\"ok\":true}}\\n'"
                ]
            }
        }),
    );
    assert_eq!(
        call_mcp_tool(&config, "fake", "ping", json!({})).unwrap(),
        json!({ "ok": true })
    );
}

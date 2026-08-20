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
                    // The client blocks on the `initialize` response before sending
                    // `notifications/initialized` or the tool call (see McpSession::start),
                    // so this must answer id 1 as soon as it's read rather than draining
                    // all three lines before replying once — otherwise it deadlocks
                    // waiting for input the client will never send.
                    "read init; printf '{\"jsonrpc\":\"2.0\",\"id\":1,\"result\":{}}\\n'; read ready; read call; printf '{\"jsonrpc\":\"2.0\",\"id\":2,\"result\":{\"ok\":true}}\\n'"
                ]
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

use super::*;
use serde_json::json;
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

fn call(name: &str, arguments: Value) -> Value {
    json!({"choices":[{"delta":{"tool_calls":[{"index":0,"id":format!("call_{name}"),"type":"function","function":{"name":name,"arguments":arguments.to_string()}}]},"finish_reason":"tool_calls"}]})
}

fn prose(text: &str, reason: &str) -> Value {
    json!({"choices":[{"delta":{"content":text},"finish_reason":reason}]})
}

async fn run_script(replies: Vec<Value>) -> (AgentResult, Vec<Value>, Vec<String>, PathBuf) {
    run_script_with_policy(replies, false, vec![]).await
}

async fn run_script_with_policy(
    replies: Vec<Value>,
    plan_mode: bool,
    disabled_tools: Vec<String>,
) -> (AgentResult, Vec<Value>, Vec<String>, PathBuf) {
    let root = std::env::temp_dir().join(format!("mint-mod-recovery-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&root).unwrap();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let config = MintConfig {
        ai_provider: "local_openai".into(),
        local_model_name: "test-native".into(),
        local_api_base_url: format!("http://{}/v1", listener.local_addr().unwrap()),
        semantic_fact_recall: false,
        memory_recall: false,
        allowed_read_paths: vec![root.clone()],
        allowed_write_paths: vec![root.clone()],
        disabled_tools,
        ..Default::default()
    };
    let recorded = Arc::new(std::sync::Mutex::new(Vec::<Value>::new()));
    let requests = Arc::clone(&recorded);
    let provider = tokio::spawn(async move {
        for reply in replies {
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut bytes = Vec::new();
            let mut chunk = [0; 4096];
            let (end, length) = loop {
                let n = socket.read(&mut chunk).await.unwrap();
                assert!(n > 0);
                bytes.extend_from_slice(&chunk[..n]);
                if let Some(end) = bytes.windows(4).position(|w| w == b"\r\n\r\n") {
                    let header = String::from_utf8_lossy(&bytes[..end]);
                    let length = header
                        .lines()
                        .find_map(|line| {
                            line.to_ascii_lowercase()
                                .strip_prefix("content-length:")
                                .and_then(|v| v.trim().parse::<usize>().ok())
                        })
                        .unwrap();
                    break (end + 4, length);
                }
            };
            while bytes.len() < end + length {
                let n = socket.read(&mut chunk).await.unwrap();
                assert!(n > 0);
                bytes.extend_from_slice(&chunk[..n]);
            }
            requests
                .lock()
                .unwrap()
                .push(serde_json::from_slice(&bytes[end..end + length]).unwrap());
            let body = format!("data: {reply}\n\ndata: [DONE]\n\n");
            socket.write_all(format!("HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len()).as_bytes()).await.unwrap();
        }
    });
    let chat = format!("mod-recovery-{}", uuid::Uuid::new_v4());
    let mut chunks = Vec::new();
    let result = tokio::time::timeout(
        Duration::from_secs(15),
        orchestrate_agent_loop(
            &config,
            "Make me a Mint mod called token-weather, in the band above the prompt.",
            &root,
            None,
            None,
            None,
            Some(&chat),
            None,
            None,
            None,
            true,
            plan_mode,
            |_| Ok(ApprovalOutcome::Denied),
            |_| {},
            |chunk| chunks.push(chunk),
        ),
    )
    .await;
    provider.abort();
    crate::mods::close_chat(&root, &chat);
    let _ = MemoryStore::open_default()
        .unwrap()
        .delete_chat_session(&chat);
    let requests = recorded.lock().unwrap().clone();
    (result.unwrap().unwrap(), requests, chunks, root)
}

#[tokio::test]
async fn truncated_mod_design_recovers_into_real_validation_and_publish() {
    let source = json!({"arguments":{
        "manifest":{"id":"token-weather","name":"Token Weather","version":"1","sdkVersion":1,"entry":"main.ts","permissions":[]},
        "source":"export function register(mint){mint.ui.set('above_composer',{type:'text',text:'☀ Clear'});}",
        "readme":"Test fixture"
    }});
    let (result, requests, chunks, root) = run_script(vec![
        call("mod_help", json!({})), call("mod_list", json!({})),
        prose("Hmm, let me reconsider the design. I'll write the files next...", "length"),
        call("mod_validate", source.clone()), call("mod_publish", source),
        call("finish", json!({"summary":"Created token-weather","verification":"mod_validate passed and mod_publish enabled the revision"})),
    ]).await;
    let published = root.join("mint-mods/token-weather/main.ts").is_file();
    std::fs::remove_dir_all(root).unwrap();
    assert!(
        published,
        "A truncated design ended the turn before publishing: {}",
        result.summary
    );
    assert_eq!(requests.len(), 6);
    assert_eq!(result.summary, "Created token-weather");
    assert!(
        !chunks.concat().contains("Hmm"),
        "Rejected prose leaked into the final answer"
    );
    let retry_messages = requests[3]["messages"].to_string();
    assert!(retry_messages.contains("mod_validate") && retry_messages.contains("mod_publish"));
}

#[tokio::test]
async fn mod_truncation_recovery_is_bounded() {
    let (result, requests, _, root) = run_script(vec![
        call("mod_help", json!({})),
        prose("Still deciding...", "length"),
        prose("Still deciding...", "length"),
        prose("Still deciding...", "length"),
    ])
    .await;
    std::fs::remove_dir_all(root).unwrap();
    assert_eq!(requests.len(), 4, "Expected at most two recovery requests");
    assert!(result.summary.contains("output-length limit"));
}

#[tokio::test]
async fn complete_mod_help_answer_does_not_trigger_recovery() {
    let (result, requests, _, root) = run_script(vec![
        call("mod_help", json!({})),
        prose("The requested capability is unavailable.", "stop"),
    ])
    .await;
    std::fs::remove_dir_all(root).unwrap();
    assert_eq!(requests.len(), 2);
    assert_eq!(result.summary, "The requested capability is unavailable.");
}

#[tokio::test]
async fn mod_recovery_respects_plan_mode_and_disabled_publication() {
    for (plan_mode, disabled_tools) in [(true, vec![]), (false, vec!["mod_publish".into()])] {
        let (result, requests, _, root) = run_script_with_policy(
            vec![
                call("mod_help", json!({})),
                prose("An incomplete draft...", "length"),
            ],
            plan_mode,
            disabled_tools,
        )
        .await;
        let published = root.join("mint-mods").exists();
        std::fs::remove_dir_all(root).unwrap();
        assert!(!published);
        assert_eq!(requests.len(), 2);
        assert!(result.summary.contains("output-length limit"));
    }
}

#[tokio::test]
async fn truncation_without_mod_preparation_keeps_normal_finish_behavior() {
    let (result, requests, _, root) =
        run_script(vec![prose("A partial answer...", "length")]).await;
    std::fs::remove_dir_all(root).unwrap();
    assert_eq!(requests.len(), 1);
    assert!(result.summary.contains("A partial answer..."));
    assert!(result.summary.contains("output-length limit"));
}

#[tokio::test]
async fn truncation_after_publication_does_not_republish() {
    let source = json!({"arguments":{
        "manifest":{"id":"token-weather","name":"Token Weather","version":"1","sdkVersion":1,"entry":"main.ts","permissions":[]},
        "source":"export function register(mint){mint.ui.set('above_composer',{type:'text',text:'☀ Clear'});}"
    }});
    let (result, requests, _, root) = run_script(vec![
        call("mod_help", json!({})),
        call("mod_publish", source),
        prose("The mod was published. More details...", "length"),
    ])
    .await;
    let published = root.join("mint-mods/token-weather/main.ts").is_file();
    std::fs::remove_dir_all(root).unwrap();
    assert!(published);
    assert_eq!(requests.len(), 3);
    assert!(result.summary.contains("output-length limit"));
}

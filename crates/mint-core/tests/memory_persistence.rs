use std::{
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};

use mint_core::{CHAT_CLI_ID, DEFAULT_CONVERSATION_ID, MemoryStore};

fn store(name: &str) -> MemoryStore {
    let path = test_path(name, "sqlite");
    let _ = std::fs::remove_file(&path);
    MemoryStore::open(path)
}

fn test_path(name: &str, extension: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    std::env::temp_dir().join(format!(
        "mint-core-integration-{name}-{}-{nanos}.{extension}",
        std::process::id(),
    ))
}

#[test]
fn stores_and_reads_profile_values() {
    let store = store("profile");
    store.set_profile("name", "Mint").unwrap();
    assert_eq!(store.get_profile("name").unwrap().as_deref(), Some("Mint"));
}

#[test]
fn stores_recent_interactions_with_provider_metadata() {
    let store = store("interactions");
    store
        .add_interaction_with_metadata("hello", "hi", "gemini", "gemini-test")
        .unwrap();
    store
        .add_interaction_for_chat_with_fallback(
            "",
            "question",
            "answer",
            "gemini",
            "gemini-test",
            Some("ollama"),
        )
        .unwrap();

    let interactions = store.recent_interactions(2).unwrap();
    assert_eq!(interactions[0].user_text, "question");
    assert_eq!(interactions[0].ai_text, "answer");
    assert_eq!(interactions[0].provider, "gemini");
    assert_eq!(interactions[0].model, "gemini-test");
    assert_eq!(interactions[0].fallback_provider, Some("ollama".to_owned()));

    assert_eq!(interactions[1].user_text, "hello");
    assert_eq!(interactions[1].fallback_provider, None);
}

#[test]
fn stores_and_reads_interaction_agent_activity() {
    let store = store("agent-activity");
    let interaction_id = store
        .add_interaction_with_metadata("hello", "hi", "gemini", "gemini-test")
        .unwrap();
    store
        .set_interaction_agent_activity_json(
            interaction_id,
            r#"[{"type":"Thought","data":{"thought":"step one"}}]"#,
        )
        .unwrap();

    let interactions = store.recent_interactions(1).unwrap();
    assert_eq!(interactions.len(), 1);
    let activity = interactions[0]
        .agent_activity
        .as_ref()
        .and_then(|value| value.as_array())
        .cloned()
        .unwrap_or_default();
    assert_eq!(activity.len(), 1);
    assert_eq!(
        activity[0].get("type").and_then(|value| value.as_str()),
        Some("Thought")
    );
}

#[test]
fn preserves_chat_history_when_store_is_reopened() {
    let path = test_path("reopen-history", "sqlite");
    let _ = std::fs::remove_file(&path);
    let first = MemoryStore::open(&path);
    first
        .add_interaction_with_metadata("persist me", "still here", "openai", "gpt-test")
        .unwrap();
    drop(first);
    let interactions = MemoryStore::open(&path).recent_interactions(10).unwrap();
    assert_eq!(interactions[0].user_text, "persist me");
    assert_eq!(interactions[0].provider, "openai");
    let _ = std::fs::remove_file(path);
}

#[test]
fn clears_interactions() {
    let store = store("clear-interactions");
    store.add_interaction("hello", "hi").unwrap();
    assert_eq!(store.clear_interactions().unwrap(), 1);
    assert!(store.recent_interactions(10).unwrap().is_empty());
}

#[test]
fn keeps_cli_history_separate_from_conversation_history() {
    let store = store("chat-session-split");
    store
        .add_interaction_for_chat(CHAT_CLI_ID, "cli question", "cli answer", "openai", "gpt")
        .unwrap();
    store
        .add_interaction_for_chat(
            "conversation-test",
            "app question",
            "app answer",
            "gemini",
            "gemini-test",
        )
        .unwrap();

    let cli = store.recent_interactions_for_chat(CHAT_CLI_ID, 10).unwrap();
    let app = store
        .recent_interactions_for_chat("conversation-test", 10)
        .unwrap();

    assert_eq!(cli.len(), 1);
    assert_eq!(cli[0].chat_id, CHAT_CLI_ID);
    assert_eq!(cli[0].user_text, "cli question");
    assert_eq!(app.len(), 1);
    assert_eq!(app[0].chat_id, "conversation-test");
    assert_eq!(app[0].user_text, "app question");
}

#[test]
fn legacy_recent_interactions_use_default_conversation() {
    let store = store("default-conversation");
    store
        .add_interaction_with_metadata("default question", "default answer", "gemini", "test")
        .unwrap();
    store
        .add_interaction_for_chat(CHAT_CLI_ID, "cli question", "cli answer", "openai", "test")
        .unwrap();

    let default_items = store.recent_interactions(10).unwrap();
    assert_eq!(default_items.len(), 1);
    assert_eq!(default_items[0].chat_id, DEFAULT_CONVERSATION_ID);
    assert_eq!(default_items[0].user_text, "default question");
}

#[test]
fn deletes_conversation_without_deleting_cli_session() {
    let store = store("delete-conversation");
    store
        .add_interaction_for_chat("conversation-delete", "remove me", "ok", "gemini", "test")
        .unwrap();
    store
        .add_interaction_for_chat(CHAT_CLI_ID, "keep me", "ok", "openai", "test")
        .unwrap();

    assert_eq!(store.delete_chat_session("conversation-delete").unwrap(), 1);
    assert!(
        store
            .recent_interactions_for_chat("conversation-delete", 10)
            .unwrap()
            .is_empty()
    );
    assert_eq!(
        store.recent_interactions_for_chat(CHAT_CLI_ID, 10).unwrap()[0].user_text,
        "keep me"
    );
    assert_eq!(store.delete_chat_session(CHAT_CLI_ID).unwrap(), 0);
}

#[test]
fn stores_workspace_session_summary() {
    let store = store("workspace-session");
    store
        .save_workspace_session("/tmp/project", "implemented", "cargo test")
        .unwrap();
    let session = store.workspace_session("/tmp/project").unwrap().unwrap();
    assert_eq!(session.summary, "implemented");
    assert_eq!(session.verification, "cargo test");
}

#[test]
fn stores_lists_and_deletes_learned_skills() {
    let store = store("skills");
    store
        .add_learned_skill("guide", "/tmp/guide.md", "Use focused patches.")
        .unwrap();
    assert_eq!(store.learned_skills(10).unwrap()[0].name, "guide");
    assert_eq!(store.delete_learned_skill("guide").unwrap(), 1);
    assert!(store.learned_skills(10).unwrap().is_empty());
}

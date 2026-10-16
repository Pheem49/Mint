//! CLI and Full-Screen TUI use the same agent execution entry point.
use std::{
    fs,
    process::Command,
    time::{SystemTime, UNIX_EPOCH},
};
#[test]
fn cli_agent_and_plain_chat_save_attachments_before_provider_failure() {
    for plain in [false, true] {
        let root = std::env::temp_dir().join(format!(
            "mint-cli-library-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&root).unwrap();
        let config = mint_core::MintConfig {
            ai_provider: "unsupported-library-test".into(),
            ..Default::default()
        };
        fs::write(
            root.join("config.json"),
            serde_json::to_vec(&config).unwrap(),
        )
        .unwrap();
        fs::write(root.join("reference.png"), b"fixture-image").unwrap();
        let mut command = Command::new(env!("CARGO_BIN_EXE_mint"));
        command
            .args(["--fast", "chat", "Inspect image", "--image"])
            .arg(root.join("reference.png"));
        if plain {
            command.args(["--system", "Describe the attached image"]);
        }
        let output = command
            .current_dir(&root)
            .env("MINT_CONFIG_PATH", root.join("config.json"))
            .env("XDG_CONFIG_HOME", root.join("config"))
            .env("XDG_DATA_HOME", root.join("data"))
            .env("XDG_CACHE_HOME", root.join("cache"))
            .env_remove("GEMINI_API_KEY")
            .env_remove("OPENAI_API_KEY")
            .env_remove("ANTHROPIC_API_KEY")
            .output()
            .unwrap();
        assert!(
            !output.status.success(),
            "unsupported provider unexpectedly succeeded"
        );
        let store = mint_core::library::LibraryStore::open(
            mint_core::MemoryStore::open(root.join("config/mint/mint-knowledge.sqlite")),
            root.join("Library"),
            root.join("Pictures"),
        )
        .unwrap();
        let page = store
            .list(&mint_core::library::LibraryQuery {
                chat_id: Some("cli".into()),
                ..Default::default()
            })
            .unwrap();
        assert_eq!(
            page.items.len(),
            1,
            "attachment missing after failed {plain:?} run: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(page.items[0].name, "reference.png");
        assert_eq!(
            store.read(&page.items[0].id, false).unwrap().1,
            b"fixture-image"
        );
        assert!(page.items[0].references[0].interaction_id.is_some());
        let _ = fs::remove_dir_all(root);
    }
}

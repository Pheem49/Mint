mod manager;
mod session;
pub use session::{Viewport, current_usage, session_snapshot, set_hot_reload, update_viewport};
mod telemetry;
pub use telemetry::UsageTracker;
mod slash;
pub use slash::slash;
mod operations;
pub use manager::*;
pub use mint_mod_runtime::worker_main;
pub use operations::{Operation, operate, operate_async};

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn discovery_finds_workspace_source_without_activation() {
        let root =
            std::env::temp_dir().join(format!("mint-mod-discovery-{}", uuid::Uuid::new_v4()));
        let dir = root.join("mint-mods/meter");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("manifest.json"),
            r#"{"id":"meter","name":"Meter","version":"1","sdkVersion":1,"entry":"main.ts"}"#,
        )
        .unwrap();
        std::fs::write(dir.join("main.ts"), "export function register(mint) {}").unwrap();
        let mods = discover(&root).unwrap();
        assert_eq!(mods.len(), 1);
        assert_eq!(mods[0]["id"], "meter");
        assert_eq!(mods[0]["enabled"], false);
        std::fs::remove_dir_all(root).unwrap();
    }
}

#[cfg(test)]
mod lifecycle_tests {
    use super::*;
    use serde_json::{Value, json};
    struct Workspace(std::path::PathBuf);
    impl Workspace {
        fn new() -> Self {
            let p = std::env::temp_dir().join(format!("mint-mods-test-{}", uuid::Uuid::new_v4()));
            std::fs::create_dir_all(&p).unwrap();
            Self(p)
        }
    }
    impl Drop for Workspace {
        fn drop(&mut self) {
            close_chat(&self.0, "a");
            close_chat(&self.0, "b");
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
    fn source(code: &str, permissions: Vec<&str>) -> Source {
        serde_json::from_value(json!({"manifest":{"id":"counter","name":"Counter","version":"1","sdkVersion":1,"entry":"main.ts","permissions":permissions},"source":code,"readme":"Example"})).unwrap()
    }
    const COUNTER: &str = r#"export function register(mint) {
        function render(){mint.ui.set('side_panel',{type:'text',text:String(mint.state.get('count')||0)});}
        mint.commands.register('bump',()=>{mint.state.set('count',(mint.state.get('count')||0)+1);render();});render();
    }"#;
    fn record(root: &std::path::Path, chat: &str) -> Value {
        list(root, chat).unwrap().remove(0)
    }
    #[test]
    fn workspace_reenable_clears_only_the_current_chats_disable_override() {
        let w = Workspace::new();
        publish(&w.0, &source(COUNTER, vec![]), None).unwrap();
        enable(&w.0, "a", "counter", "workspace", false).unwrap();
        disable(&w.0, "a", "counter").unwrap();
        disable(&w.0, "b", "counter").unwrap();
        enable(&w.0, "a", "counter", "workspace", false).unwrap();
        assert_eq!(record(&w.0, "a")["enabled"], true);
        assert_eq!(record(&w.0, "b")["enabled"], false);
    }
    #[test]
    fn hot_reload_preserves_state_and_changes_only_the_opted_in_chat() {
        let w = Workspace::new();
        let s = source(COUNTER, vec![]);
        publish(&w.0, &s, None).unwrap();
        enable(&w.0, "a", "counter", "workspace", false).unwrap();
        command(&w.0, "a", "counter", "bump", "").unwrap();
        let old = record(&w.0, "a")["instanceId"].clone();
        record(&w.0, "b");
        set_hot_reload(&w.0, "a", true).unwrap();
        std::fs::write(
            w.0.join("mint-mods/counter/main.ts"),
            COUNTER.replace(
                "String(mint.state.get('count')||0)",
                "'Reloaded '+String(mint.state.get('count')||0)",
            ),
        )
        .unwrap();
        let current = record(&w.0, "a");
        assert_eq!(
            current["snapshot"]["ui"]["side_panel"]["text"],
            "Reloaded 1"
        );
        assert_ne!(current["instanceId"], old);
        assert_eq!(
            record(&w.0, "b")["snapshot"]["ui"]["side_panel"]["text"],
            "0"
        );
        assert!(
            action(
                &w.0,
                "a",
                "counter",
                old.as_str().unwrap(),
                "bump",
                Value::Null
            )
            .is_err()
        );
    }
    #[test]
    fn invalid_hot_reload_keeps_the_active_worker_and_privileges_require_review() {
        let w = Workspace::new();
        let s = source(COUNTER, vec![]);
        publish(&w.0, &s, None).unwrap();
        enable(&w.0, "a", "counter", "chat", false).unwrap();
        set_hot_reload(&w.0, "a", true).unwrap();
        std::fs::write(
            w.0.join("mint-mods/counter/main.ts"),
            "export function register(mint){throw Error('Bad revision');}",
        )
        .unwrap();
        let m = record(&w.0, "a");
        assert!(m["reloadError"].as_str().is_some());
        assert!(m["error"].is_null());
        command(&w.0, "a", "counter", "bump", "").unwrap();
        assert_eq!(
            record(&w.0, "a")["snapshot"]["ui"]["side_panel"]["text"],
            "1"
        );
        std::fs::write(w.0.join("mint-mods/counter/main.ts"), COUNTER).unwrap();
        assert!(record(&w.0, "a")["reloadError"].is_null());
        std::fs::write(
            w.0.join("mint-mods/counter/main.ts"),
            "export function register(mint){throw Error('Bad revision');}",
        )
        .unwrap();
        assert!(record(&w.0, "a")["reloadError"].as_str().is_some());
        let privileged = source(COUNTER, vec!["workspace.read"]);
        publish(
            &w.0,
            &privileged,
            Some(&load_source(&w.0, "counter").unwrap().hash()),
        )
        .unwrap();
        let m = record(&w.0, "a");
        assert_eq!(m["activeHash"], s.hash());
        assert!(m["reloadError"].as_str().unwrap().contains("review"));
    }
    #[test]
    fn sdk_getters_read_latest_usage_and_resize_redraws_without_a_turn() {
        let w = Workspace::new();
        event(
            &w.0,
            "a",
            "usage_update",
            json!({"context":{"tokens":36100}}),
            None,
        )
        .unwrap();
        let s = source(
            "export function register(mint){function draw(){mint.ui.set('above_composer',{type:'text',text:mint.session.usage().context.tokens+' / '+mint.ui.viewport().columns});}draw();mint.on('ui_resize',draw);}",
            vec![],
        );
        publish(&w.0, &s, None).unwrap();
        enable(&w.0, "a", "counter", "chat", false).unwrap();
        update_viewport(
            &w.0,
            "a",
            Viewport {
                surface: Some("terminal".into()),
                columns: Some(80),
                rows: Some(30),
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(
            record(&w.0, "a")["snapshot"]["ui"]["above_composer"]["text"],
            "36100 / 80"
        );
        assert!(current_usage(&w.0, "b")["context"]["tokens"].is_null());
        close_chat(&w.0, "a");
        assert_eq!(current_usage(&w.0, "a")["context"]["tokens"], 36100);
        assert_eq!(session_snapshot(&w.0, "a").unwrap()["hotReload"], false);
    }
    #[test]
    fn activation_state_and_disk_changes_are_separate_per_chat() {
        let w = Workspace::new();
        let s = source(COUNTER, vec![]);
        publish(&w.0, &s, None).unwrap();
        assert_eq!(record(&w.0, "a")["enabled"], false);
        enable(&w.0, "a", "counter", "chat", false).unwrap();
        let before = record(&w.0, "a");
        let instance = before["instanceId"].as_str().unwrap();
        action(&w.0, "a", "counter", instance, "bump", Value::Null).unwrap();
        assert_eq!(
            record(&w.0, "a")["snapshot"]["ui"]["side_panel"]["text"],
            "1"
        );
        assert_eq!(record(&w.0, "b")["enabled"], false);
        enable(&w.0, "b", "counter", "chat", false).unwrap();
        assert_eq!(
            record(&w.0, "b")["snapshot"]["ui"]["side_panel"]["text"],
            "0"
        );
        assert_eq!(record(&w.0, "a")["enabled"], true);
        close_chat(&w.0, "a");
        assert_eq!(
            record(&w.0, "a")["snapshot"]["ui"]["side_panel"]["text"],
            "1"
        );
        assert!(action(&w.0, "a", "counter", instance, "bump", Value::Null).is_err());
        std::fs::write(w.0.join("mint-mods/counter/main.ts"),"export function register(mint) {mint.ui.set('side_panel',{type:'text',text:'changed'});}").unwrap();
        assert_eq!(record(&w.0, "a")["changed"], true);
        assert_eq!(
            record(&w.0, "a")["snapshot"]["ui"]["side_panel"]["text"],
            "1"
        );
    }
    #[test]
    fn permissions_are_reapproved_for_changed_content_and_failed_hooks_stay_closed() {
        let w = Workspace::new();
        let s = source(COUNTER, vec!["workspace.read"]);
        publish(&w.0, &s, None).unwrap();
        assert!(enable(&w.0, "a", "counter", "chat", false).is_err());
        enable(&w.0, "a", "counter", "chat", true).unwrap();
        let failing = source(
            "export function register(mint){mint.on('pre_tool_use',()=>{while(true){}});}",
            vec![],
        );
        publish(&w.0, &failing, Some(&s.hash())).unwrap();
        enable(&w.0, "a", "counter", "chat", false).unwrap();
        assert!(
            event(
                &w.0,
                "a",
                "pre_tool_use",
                json!({"action":"read_file"}),
                None
            )
            .is_err()
        );
        assert!(
            event(
                &w.0,
                "a",
                "pre_tool_use",
                json!({"action":"read_file"}),
                None
            )
            .is_err()
        );
        disable(&w.0, "a", "counter").unwrap();
        assert!(
            event(
                &w.0,
                "a",
                "pre_tool_use",
                json!({"action":"read_file"}),
                None
            )
            .is_ok()
        );
    }
    #[test]
    fn publishing_checks_disk_hash_and_rollback_restores_source() {
        let w = Workspace::new();
        let s = source(COUNTER, vec![]);
        let hash = publish(&w.0, &s, None).unwrap();
        let changed = source("export function register(mint) {}", vec![]);
        assert!(publish(&w.0, &changed, Some("wrong")).is_err());
        assert_eq!(load_source(&w.0, "counter").unwrap().source, COUNTER);
        let next = publish(&w.0, &changed, Some(&hash)).unwrap();
        rollback(&w.0, "counter", &hash, &next).unwrap();
        assert_eq!(load_source(&w.0, "counter").unwrap().source, COUNTER);
    }
    #[cfg(unix)]
    #[test]
    fn disabled_source_discovery_cannot_block_agent_tools() {
        let w = Workspace::new();
        std::os::unix::fs::symlink("/tmp", w.0.join("mint-mods")).unwrap();
        assert!(
            event(
                &w.0,
                "a",
                "pre_tool_use",
                json!({"action":"read_file"}),
                None
            )
            .is_ok()
        );
    }
    #[test]
    fn shutdown_releases_chat_instances_and_saves_session_end_state() {
        let w = Workspace::new();
        let chat = uuid::Uuid::new_v4().to_string();
        let s = source(
            "export function register(mint){mint.on('session_end',()=>mint.state.set('closed',true));mint.ui.set('side_panel',{type:'text',text:String(mint.state.get('closed')||false)});}",
            vec![],
        );
        publish(&w.0, &s, None).unwrap();
        enable(&w.0, &chat, "counter", "chat", false).unwrap();
        let old = record(&w.0, &chat)["instanceId"].clone();
        shutdown_chat(&chat);
        let reopened = record(&w.0, &chat);
        assert_ne!(reopened["instanceId"], old);
        assert_eq!(reopened["snapshot"]["ui"]["side_panel"]["text"], "true");
        close_chat(&w.0, &chat);
    }
    #[test]
    fn pre_tool_hooks_cannot_bypass_sdk_to_enqueue_tools() {
        let w = Workspace::new();
        let s = source(
            r#"export function register(mint){mint.on('pre_tool_use',()=>{__host(JSON.stringify({kind:'tool',action:'read_file',input:{path:'README.md'}}));});}"#,
            vec!["tools.request"],
        );
        publish(&w.0, &s, None).unwrap();
        enable(&w.0, "a", "counter", "chat", true).unwrap();
        event(
            &w.0,
            "a",
            "pre_tool_use",
            json!({"action":"read_file"}),
            None,
        )
        .unwrap();
        assert!(pending_requests(&w.0, "a").unwrap().is_empty());
    }
    #[cfg(unix)]
    #[test]
    fn symlinked_sources_and_workspace_escape_are_rejected() {
        let w = Workspace::new();
        let s = source(COUNTER, vec![]);
        publish(&w.0, &s, None).unwrap();
        let entry = w.0.join("mint-mods/counter/main.ts");
        std::fs::remove_file(&entry).unwrap();
        std::os::unix::fs::symlink("/etc/passwd", &entry).unwrap();
        assert!(load_source(&w.0, "counter").is_err());
    }
}

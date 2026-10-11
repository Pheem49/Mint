use super::super::*;

pub(in crate::orchestration) async fn execute(
    action: &str,
    input: &AgentInput,
    root: &Path,
    config: &MintConfig,
    chat: &str,
    approve: &mut (dyn FnMut(&AgentApproval) -> Result<ApprovalOutcome, String> + Send),
) -> Result<String, OrchestrationError> {
    let result: Result<String, String> = (|| {
        if action == "mod_help" {
            return Ok(include_str!("../../../../../docs/mods/AUTHORING.md").into());
        }
        if action == "mod_list" {
            return Ok(serde_json::json!({"mods":crate::mods::list(root,chat)?}).to_string());
        }
        let source: crate::mods::Source =
            serde_json::from_value(input.arguments.clone()).map_err(|e| e.to_string())?;
        let draft = crate::mods::save_draft(root, &source)?;
        let preview = crate::mods::validate(root, &source)
            .map_err(|error| format!("{error}\nDraft saved at {}", draft.display()))?;
        if action == "mod_validate" {
            return Ok(serde_json::json!({"valid":true,"preview":preview}).to_string());
        }
        crate::assert_path_capability(&root.join("mint-mods"), crate::Capability::Write, config)
            .map_err(|e| e.to_string())?;
        let expected = input.arguments["expectedHash"].as_str();
        let hash = crate::mods::publish(root, &source, expected)?;
        let granted = if source.manifest.permissions.is_empty() {
            false
        } else {
            let approval = AgentApproval::UserApproval {
                title: format!("Enable {}", source.manifest.name),
                prompt: format!(
                    "Mod: {}\nRevision: {hash}\nPermissions: {}\n\n```typescript\n{}\n```",
                    source.manifest.id,
                    source.manifest.permissions.join(", "),
                    source.source
                ),
            };
            matches!(approve(&approval)?, ApprovalOutcome::Approved)
        };
        if source.manifest.permissions.is_empty() || granted {
            crate::mods::enable_reviewed(
                root,
                chat,
                &source.manifest.id,
                "chat",
                granted,
                Some(&hash),
            )?;
        }
        Ok(serde_json::json!({"path":root.join("mint-mods").join(&source.manifest.id),"hash":hash,"validated":true,"enabled":source.manifest.permissions.is_empty()||granted}).to_string())
    })();
    result.map_err(OrchestrationError::Agent)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn mod_requests_respect_plan_mode_disabled_tools_and_deliver_results() {
        let root = std::env::temp_dir().join(format!("mint-mod-policy-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&root).unwrap();
        let chat = uuid::Uuid::new_v4().to_string();
        let config = MintConfig {
            allowed_read_paths: vec![root.clone()],
            allowed_write_paths: vec![root.clone()],
            disabled_tools: vec!["read_file".into()],
            ..Default::default()
        };
        let source: crate::mods::Source = serde_json::from_value(serde_json::json!({"manifest":{"id":"policy-test","name":"Policy test","version":"1","sdkVersion":1,"entry":"main.ts","permissions":["tools.request"]},"source":"export function register(mint){mint.on('session_start',()=>{mint.tools.request('write_file',{path:'should-not-exist',content:'bad'});mint.tools.request('read_file',{path:'README.md'});});mint.on('tool_result',r=>{if(!r.error)throw Error('Expected policy rejection');mint.state.set('results',(mint.state.get('results')||0)+1);});}"})).unwrap();
        crate::mods::publish(&root, &source, None).unwrap();
        crate::mods::enable(&root, &chat, "policy-test", "chat", true).unwrap();
        crate::mods::set_turn_policy(&root, &chat, true, None);
        execute_mod_requests(
            &root,
            &chat,
            &config,
            false,
            &mut |_| panic!("Policy rejection must precede approvals"),
            &mut |_| {},
            None,
        )
        .await
        .unwrap();
        assert!(!root.join("should-not-exist").exists());
        let mods = crate::mods::list(&root, &chat).unwrap();
        assert_eq!(mods[0]["snapshot"]["state"]["results"], 2);
        assert!(
            crate::mods::pending_requests(&root, &chat)
                .unwrap()
                .is_empty()
        );
        crate::mods::close_chat(&root, &chat);
        std::fs::remove_dir_all(root).unwrap();
    }
    #[tokio::test]
    async fn authoring_tools_preserve_failed_drafts_and_publish_a_live_mod() {
        let root =
            std::env::temp_dir().join(format!("mint-mod-authoring-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&root).unwrap();
        let chat = uuid::Uuid::new_v4().to_string();
        let config = MintConfig {
            allowed_read_paths: vec![root.clone()],
            allowed_write_paths: vec![root.clone()],
            ..Default::default()
        };
        let manifest = serde_json::json!({"id":"prompt-test","name":"Prompt test","version":"1","sdkVersion":1,"entry":"main.ts","permissions":[]});
        let invalid: crate::mods::Source = serde_json::from_value(
            serde_json::json!({"manifest":manifest,"source":"export ???","readme":"Draft"}),
        )
        .unwrap();
        let saved = crate::mods::save_draft(&root, &invalid).unwrap();
        let mut input = AgentInput::default();
        input.arguments = serde_json::to_value(&invalid).unwrap();
        let mut approve = |_: &AgentApproval| panic!("Basic mod requires no permission prompt");
        assert!(
            execute("mod_validate", &input, &root, &config, &chat, &mut approve)
                .await
                .is_err()
        );
        assert!(saved.is_file());
        assert!(!root.join("mint-mods/prompt-test").exists());
        input.arguments["source"] = serde_json::json!(
            "export async function register(mint){await Promise.resolve();mint.ui.set('above_composer',{type:'text',text:'Created from prompt'});}"
        );
        let validation = execute("mod_validate", &input, &root, &config, &chat, &mut approve)
            .await
            .unwrap();
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(&validation).unwrap()["valid"],
            true
        );
        let result = execute("mod_publish", &input, &root, &config, &chat, &mut approve)
            .await
            .unwrap();
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(&result).unwrap()["enabled"],
            true
        );
        let mods = crate::mods::list(&root, &chat).unwrap();
        assert_eq!(
            mods[0]["snapshot"]["ui"]["above_composer"]["text"],
            "Created from prompt"
        );
        crate::mods::close_chat(&root, &chat);
        std::fs::remove_dir_all(root).unwrap();
    }
}

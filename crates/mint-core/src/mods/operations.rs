use super::*;
use base64::{Engine as _, engine::general_purpose::STANDARD};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::path::Path;

#[derive(Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Operation {
    pub workspace_path: String,
    pub chat_id: String,
    pub action: String,
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub scope: String,
    #[serde(default)]
    pub grant_permissions: bool,
    #[serde(default)]
    pub expected_hash: Option<String>,
    #[serde(default)]
    pub hash: String,
    #[serde(default)]
    pub instance_id: String,
    #[serde(default)]
    pub command: String,
    #[serde(default)]
    pub value: Value,
    #[serde(default)]
    pub source: Option<Source>,
    #[serde(default)]
    pub package: String,
    #[serde(default)]
    pub viewport: Option<Viewport>,
}
pub fn operate(op: &Operation) -> Result<Value, String> {
    if op.chat_id.trim().is_empty() || op.chat_id.len() > 512 {
        return Err("An explicit chat ID is required".into());
    }
    let root = Path::new(&op.workspace_path);
    let config = crate::load_config().map_err(|e| e.to_string())?;
    crate::assert_path_capability(root, crate::Capability::Read, &config)
        .map_err(|e| e.to_string())?;
    let chat = crate::scoped_chat_id(&op.chat_id, Some(&op.workspace_path));
    match op.action.as_str() {
        "list" => {
            return Ok(
                json!({"mods":list(root,&chat)?,"toolRequests":pending_requests(root,&chat)?,"session":session_snapshot(root,&chat)?}),
            );
        }
        "validate" => {
            return validate(
                root,
                &match &op.source {
                    Some(s) => s.clone(),
                    None => shareable_source(root, &op.id)?,
                },
            );
        }
        "hot_reload" => set_hot_reload(
            root,
            &chat,
            op.value
                .as_bool()
                .ok_or("Hot reload requires true or false")?,
        )?,
        "viewport" => {
            update_viewport(root, &chat, op.viewport.clone().ok_or("Viewport required")?)?
        }
        "usage" => return Ok(json!({"usage":current_usage(root,&chat)})),
        "test" => return Ok(json!({"testReport":test_mod(root,&op.id)?})),
        "enable" => {
            if op.grant_permissions && op.expected_hash.is_none() {
                return Err("Review the source hash before granting mod permissions".into());
            }
            enable_reviewed(
                root,
                &chat,
                &op.id,
                if op.scope.is_empty() {
                    "chat"
                } else {
                    &op.scope
                },
                op.grant_permissions,
                op.expected_hash.as_deref(),
            )?;
        }
        "disable" => disable(root, &chat, &op.id)?,
        "action" => {
            return action(
                root,
                &chat,
                &op.id,
                &op.instance_id,
                &op.command,
                op.value.clone(),
            );
        }
        "command" => {
            return command(
                root,
                &chat,
                &op.id,
                &op.command,
                op.value.as_str().unwrap_or(""),
            );
        }
        "close" => {
            close_chat(root, &chat);
            return Ok(json!({"mods":[],"session":session_snapshot(root,&chat)?}));
        }
        "rollback" => {
            crate::assert_path_capability(
                &source_path(root, &op.id)?,
                crate::Capability::Write,
                &config,
            )
            .map_err(|e| e.to_string())?;
            rollback(
                root,
                &op.id,
                &op.hash,
                op.expected_hash
                    .as_deref()
                    .ok_or("Expected source hash required")?,
            )?;
        }
        "inspect_package" => {
            if op.package.len() > 2 * 1024 * 1024 {
                return Err("Package is too large".into());
            }
            let bytes = STANDARD.decode(&op.package).map_err(|e| e.to_string())?;
            let source: Source = serde_json::from_value(mint_mod_runtime::unpack(&bytes)?)
                .map_err(|e| e.to_string())?;
            validate(root, &source)?;
            let expected = load_source(root, &source.manifest.id)
                .ok()
                .map(|s| s.hash());
            return Ok(json!({"id":source.manifest.id,"expectedHash":expected}));
        }
        "export" => {
            let source = shareable_source(root, &op.id)?;
            let bytes = mint_mod_runtime::pack(
                &serde_json::to_value(&source.manifest).unwrap(),
                &source.source,
                &source.readme,
            )?;
            return Ok(
                json!({"filename":format!("{}.mintmod",op.id),"package":STANDARD.encode(bytes)}),
            );
        }
        "import" => {
            if op.package.len() > 2 * 1024 * 1024 {
                return Err("Package is too large".into());
            }
            let bytes = STANDARD.decode(&op.package).map_err(|e| e.to_string())?;
            let source: Source = serde_json::from_value(mint_mod_runtime::unpack(&bytes)?)
                .map_err(|e| e.to_string())?;
            crate::assert_path_capability(
                &source_path(root, &source.manifest.id)?,
                crate::Capability::Write,
                &config,
            )
            .map_err(|e| e.to_string())?;
            publish(root, &source, op.expected_hash.as_deref())?;
        }
        _ => return Err("Unknown mod operation".into()),
    }
    Ok(json!({"mods":list(root,&chat)?,"session":session_snapshot(root,&chat)?}))
}

pub async fn operate_async(op: Operation) -> Result<Value, String> {
    if op.action == "run_request" {
        let config = crate::load_config().map_err(|e| e.to_string())?;
        let root = crate::assert_path_capability(
            Path::new(&op.workspace_path),
            crate::Capability::Read,
            &config,
        )
        .map_err(|e| e.to_string())?;
        if op.chat_id.is_empty() || !op.grant_permissions {
            return Err("Review and approve the concrete tool request first".into());
        }
        let chat = crate::scoped_chat_id(&op.chat_id, Some(&op.workspace_path));
        let ids = vec![op.command.clone()];
        crate::orchestration::execute_mod_requests(
            &root,
            &chat,
            &config,
            false,
            &mut |_| Ok(crate::ApprovalOutcome::Approved),
            &mut |_| {},
            Some(&ids),
        )
        .await?;
        return Ok(
            json!({"mods":list(&root,&chat)?,"toolRequests":pending_requests(&root,&chat)?}),
        );
    }
    tokio::task::spawn_blocking(move || operate(&op))
        .await
        .map_err(|e| e.to_string())?
}

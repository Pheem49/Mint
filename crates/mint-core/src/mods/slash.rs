use super::*;
use serde_json::json;
use std::io::Read;
use std::path::Path;

pub fn slash(root: &Path, chat: &str, input: &str) -> Result<String, String> {
    let chat = crate::scoped_chat_id(chat, Some(&root.to_string_lossy()));
    if let Some(rest) = input.strip_prefix("/mods:") {
        let (token, args) = rest.split_once(char::is_whitespace).unwrap_or((rest, ""));
        let (id, name) = token.split_once(':').ok_or("Use /mods:<id>:<command>")?;
        return command(root, &chat, id, name, args.trim()).map(|r| {
            r["value"]
                .as_str()
                .map(str::to_owned)
                .unwrap_or_else(|| r["value"].to_string())
        });
    }
    let rest = input.trim().strip_prefix("/mods").unwrap_or(input).trim();
    let words = shlex::split(rest).ok_or("Unclosed quote in mod command")?;
    let mut args = words.iter().map(String::as_str);
    let action = args.next().unwrap_or("list");
    let id = args.next().unwrap_or("");
    if matches!(action, "import" | "rollback") {
        crate::assert_path_capability(
            &root.join("mint-mods"),
            crate::Capability::Write,
            &crate::load_config().map_err(|e| e.to_string())?,
        )
        .map_err(|e| e.to_string())?;
    }
    match action {
        "hot-reload"=>{let enabled=match id {"on"=>true,"off"=>false,""=>return Ok(session_snapshot(root,&chat)?.to_string()),_=>return Err("Use /mods hot-reload on|off".into())};set_hot_reload(root,&chat,enabled)?;Ok(format!("Hot reload {} for this session",if enabled {"on"}else{"off"}))},
        "usage"=>Ok(current_usage(root,&chat).to_string()),
        "test"=>Ok(test_mod(root,id)?.to_string()),
        "export" => {
            let options: Vec<_> = args.collect();
            let default = format!("{id}.mintmod");
            let filename = options.first().copied().filter(|s| !s.starts_with("--")).unwrap_or(&default);
            let path = crate::assert_path_capability(&root.join(filename), crate::Capability::Write, &crate::load_config().map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
            if path.exists() && !options.contains(&"--replace") { return Err("Export file exists; choose another path or add --replace".into()); }
            let source = shareable_source(root, id)?;
            let bytes = mint_mod_runtime::pack(&serde_json::to_value(&source.manifest).map_err(|e| e.to_string())?, &source.source, &source.readme)?;
            std::fs::write(&path, bytes).map_err(|e| e.to_string())?;
            Ok(format!("Exported {}", path.display()))
        },
        "import" => {
            let path = crate::assert_path_capability(&root.join(id), crate::Capability::Read, &crate::load_config().map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
            let mut bytes = Vec::new();
            std::fs::File::open(path).map_err(|e| e.to_string())?.take(1024 * 1024 + 1).read_to_end(&mut bytes).map_err(|e| e.to_string())?;
            let source: Source = serde_json::from_value(mint_mod_runtime::unpack(&bytes)?).map_err(|e| e.to_string())?;
            let replace = args.any(|a| a == "--replace");
            let expected = if replace { load_source(root, &source.manifest.id).ok().map(|s| s.hash()) } else { None };
            publish(root, &source, expected.as_deref())?;
            Ok(format!("Imported {}; disabled until enabled", source.manifest.id))
        },
        "list"=>{let mods=list(root,&chat)?;if mods.is_empty(){return Ok("No mods yet. Ask Mint to create one in mint-mods/.".into());}Ok(mods.iter().map(|m|format!("- {} · {}{}",m["id"].as_str().unwrap_or(""),if m["enabled"]==true{"active"}else{"off"},m["error"].as_str().or(m["reloadError"].as_str()).map(|e|format!(" · {e}")).unwrap_or_default())).collect::<Vec<_>>().join("\n"))},
        "source"=>{let source=shareable_source(root,id)?;Ok(format!("```json\n{}\n```\n```typescript\n{}\n```",serde_json::to_string_pretty(&source.manifest).unwrap(),source.source))},
        "logs"=>{let mods=list(root,&chat)?;let m=mods.iter().find(|m|m["id"].as_str()==Some(id)).ok_or("Unknown mod")?;Ok(m["snapshot"]["logs"].to_string())},
        "validate"=>{let source=shareable_source(root,id)?;validate(root,&source).map(|s|json!({"valid":true,"preview":s}).to_string())},
        "enable"=>{let options:Vec<_>=args.collect();let scope=options.iter().find(|a|["chat","workspace","global"].contains(a)).copied().unwrap_or("chat");enable(root,&chat,id,scope,options.contains(&"--grant-permissions"))?;Ok(format!("Enabled {id} for {scope}"))},
        "disable"=>{disable(root,&chat,id)?;Ok(format!("Disabled {id} in this chat"))},
        "revisions"=>Ok(serde_json::to_string_pretty(&revisions(root,id)?).unwrap()),
        "rollback"=>{let hash=args.next().ok_or("Revision hash required")?;let current=load_source(root,id)?;rollback(root,id,hash,&current.hash())?;Ok(format!("Restored {id} source; review and enable the revision."))},
        _=>Ok("/mods list | source <id> | validate <id> | test <id> | hot-reload on|off | usage | enable <id> [chat|workspace|global] [--grant-permissions] | disable <id> | logs <id> | revisions <id> | rollback <id> <hash> | export <id> [file] [--replace] | import <file> [--replace]\n/mods:<id>:<command> [value]".into())
    }
}

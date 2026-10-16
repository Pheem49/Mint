use anyhow::{Result, bail};

pub(crate) async fn run(chat: &str, args: &[String]) -> Result<()> {
    let root = std::env::current_dir()?;
    let action = args.first().map(String::as_str).unwrap_or("list");
    let id = args.get(1).map(String::as_str).unwrap_or("");
    match action {
        "test" => {
            let report = mint_core::mods::test_mod(&root, id).map_err(anyhow::Error::msg)?;
            println!("{}", serde_json::to_string_pretty(&report)?);
            if report["failed"].as_u64().unwrap_or(0) > 0 {
                bail!("Mod tests failed");
            }
        }
        "hot-reload" => bail!("Use /mods hot-reload on|off inside an interactive Mint session"),
        "run-tools" => {
            let chat = mint_core::scoped_chat_id(chat, Some(&root.to_string_lossy()));
            let requests =
                mint_core::mods::pending_requests(&root, &chat).map_err(anyhow::Error::msg)?;
            for request in requests {
                println!(
                    "{} requests {}\n{}\nRun this request? [y/N]",
                    request["modId"],
                    request["action"],
                    serde_json::to_string_pretty(&request["input"])?
                );
                let mut answer = String::new();
                std::io::stdin().read_line(&mut answer)?;
                if !answer.trim().eq_ignore_ascii_case("y") {
                    continue;
                }
                let op = serde_json::from_value(
                    serde_json::json!({"workspacePath":root,"chatId":chat,"action":"run_request","command":request["id"],"grantPermissions":true}),
                )?;
                mint_core::mods::operate_async(op)
                    .await
                    .map_err(anyhow::Error::msg)?;
            }
        }
        _ => {
            if action == "enable" && id.is_empty() {
                bail!("Usage: mint mods enable <id> [chat|workspace|global] [--grant-permissions]");
            }
            println!(
                "{}",
                mint_core::mods::slash(
                    &root,
                    chat,
                    &format!(
                        "/mods {}",
                        args.iter()
                            .map(|arg| format!("'{}'", arg.replace('\'', "'\"'\"'")))
                            .collect::<Vec<_>>()
                            .join(" ")
                    )
                )
                .map_err(anyhow::Error::msg)?
            );
        }
    }
    Ok(())
}

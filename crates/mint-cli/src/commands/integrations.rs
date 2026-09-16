use std::path::PathBuf;

use anyhow::Result;
use clap::Subcommand;
use mint_core::{add_linked_folder, list_linked_folders, remove_linked_folder};

use crate::{
    BLUE, DIM, ERROR, MINT, RESET, WARN, gmail, hooks, mcp, plugins_cli, print_mcp_servers,
};

#[derive(Debug, Subcommand)]
pub enum McpCommand {
    Add {
        name: String,
        command: String,
        /// Repeatable: one value per `--args` occurrence (e.g. `--args -y --args --header`).
        #[arg(long, num_args = 1, allow_hyphen_values = true)]
        args: Vec<String>,
        #[arg(long, num_args = 1, allow_hyphen_values = true)]
        env: Vec<String>,
        /// Immediately allow the agent to call every tool on the new server
        /// (`allow <name> *`). Off by default — tools are opt-in per server.
        #[arg(long)]
        allow_all: bool,
    },
    List,
    Remove {
        name: String,
    },
    /// Keep a server configured but turn it off (hidden from the agent).
    Disable {
        name: String,
    },
    /// Re-enable a server turned off with `disable`.
    Enable {
        name: String,
    },
    /// Change a configured server in place — only the flags you pass are touched.
    Edit {
        name: String,
        #[arg(long)]
        command: Option<String>,
        /// Repeatable; replaces the arg list. Passing none leaves args unchanged.
        #[arg(long, num_args = 1, allow_hyphen_values = true)]
        args: Vec<String>,
        /// Repeatable `KEY=VALUE`; replaces the env map. None = unchanged.
        #[arg(long, num_args = 1, allow_hyphen_values = true)]
        env: Vec<String>,
        #[arg(long)]
        icon: Option<String>,
        /// Clear the server's icon.
        #[arg(long, conflicts_with = "icon")]
        no_icon: bool,
    },
    Allow {
        server: String,
        tool: String,
    },
    /// Remove a tool from a server's allowlist (`*` clears the whole list).
    Disallow {
        server: String,
        tool: String,
    },
    /// Re-run a server's OAuth flow in the foreground.
    Reauth {
        server: String,
    },
    Clear,
    Call {
        server: String,
        tool: String,
        #[arg(long, default_value = "{}")]
        arguments: String,
    },
    /// The bundled catalog of well-known MCP servers.
    Registry {
        #[command(subcommand)]
        command: Option<RegistryCommand>,
    },
}

#[derive(Debug, Subcommand)]
pub enum RegistryCommand {
    /// List the catalog (the default when no subcommand is given).
    List,
    /// Add a catalog entry as a configured server.
    Add {
        /// Catalog key, e.g. `filesystem` (see `mint mcp registry`).
        key: String,
        /// Server name to save it under. Defaults to the catalog key.
        #[arg(long)]
        name: Option<String>,
        /// One value per `argInput` the entry declares, in order (e.g. a path).
        #[arg(long = "arg", num_args = 1, allow_hyphen_values = true)]
        args: Vec<String>,
        /// `KEY=VALUE` for the entry's required env vars.
        #[arg(long, num_args = 1, allow_hyphen_values = true)]
        env: Vec<String>,
        /// Immediately allow the agent to call every tool on it.
        #[arg(long)]
        allow_all: bool,
    },
}

#[derive(Debug, Subcommand)]
pub enum LinkCommand {
    Add {
        name: String,
        path: PathBuf,
        #[arg(long)]
        description: Option<String>,
    },
    List,
    Remove {
        name: String,
    },
}

#[derive(Debug, Subcommand)]
pub enum HooksCommand {
    /// Add a hook. event: PreToolUse or PostToolUse.
    Add {
        event: String,
        matcher: String,
        command: String,
        #[arg(long)]
        timeout: Option<u64>,
    },
    List,
    Remove {
        index: usize,
    },
    Clear,
}

#[derive(Debug, Subcommand)]
pub enum GmailCommand {
    Auth {
        #[arg(long)]
        no_open: bool,
        #[arg(long, default_value_t = 0)]
        port: u16,
    },
}

pub async fn handle_mcp(command: McpCommand) -> Result<()> {
    match command {
        McpCommand::Add {
            name,
            command,
            args,
            env,
            allow_all,
        } => {
            if command.starts_with("http://") || command.starts_with("https://") {
                let headers = if env.is_empty() {
                    None
                } else {
                    let mut map = std::collections::BTreeMap::new();
                    for item in env {
                        if let Some((k, v)) = item.split_once('=') {
                            map.insert(k.to_string(), v.to_string());
                        }
                    }
                    Some(map)
                };
                mcp::add_remote(&name, &command, headers)?;
                println!("Added Remote MCP server: {name} ({command})");
            } else {
                mcp::add(&name, &command, args, env)?;
                println!("Added MCP server: {name}");
            }
            if allow_all {
                mcp::allow(&name, "*")?;
                println!("Allowed all tools for {name}");
            }
        }
        McpCommand::List => {
            println!("\n{BLUE}MCP servers:{RESET}");
            print_mcp_servers(&mcp::list()?);
        }
        McpCommand::Remove { name } => {
            println!(
                "{}",
                if mcp::remove(&name)? {
                    "removed"
                } else {
                    "not found"
                }
            )
        }
        McpCommand::Disable { name } => {
            if mcp::set_disabled(&name, true)? {
                println!("disabled {name}");
            } else {
                println!("{ERROR}not found:{RESET} {name}");
            }
        }
        McpCommand::Enable { name } => {
            if mcp::set_disabled(&name, false)? {
                println!("enabled {name}");
            } else {
                println!("{ERROR}not found:{RESET} {name}");
            }
        }
        McpCommand::Edit {
            name,
            command,
            args,
            env,
            icon,
            no_icon,
        } => {
            let icon = if no_icon { Some(None) } else { icon.map(Some) };
            let existed = mcp::edit(
                &name,
                command,
                (!args.is_empty()).then_some(args),
                (!env.is_empty()).then_some(env),
                icon,
            )?;
            if existed {
                println!("updated {name}");
            } else {
                println!("{ERROR}not found:{RESET} {name}");
            }
        }
        McpCommand::Allow { server, tool } => {
            if mcp::allow(&server, &tool)? {
                println!("allowed {server}/{tool}");
            } else {
                println!("already allowed {server}/{tool}");
            }
        }
        McpCommand::Disallow { server, tool } => {
            if mcp::disallow(&server, &tool)? {
                println!("disallowed {server}/{tool}");
            } else {
                println!("{server}/{tool} was not in the allowlist");
            }
        }
        McpCommand::Reauth { server } => {
            println!("Re-authenticating MCP server '{server}'...");
            if mcp::reauth(&server)? {
                println!("Re-authentication succeeded for '{server}'.");
            } else {
                println!("Re-authentication failed for '{server}' (see output above).");
            }
        }
        McpCommand::Clear => {
            mcp::clear()?;
            println!("cleared");
        }
        McpCommand::Call {
            server,
            tool,
            arguments,
        } => {
            use indicatif::{ProgressBar, ProgressStyle};
            use std::time::Duration;
            let spinner = ProgressBar::new_spinner();
            spinner.set_style(
                ProgressStyle::default_spinner()
                    .tick_chars("⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏")
                    .template("{spinner:.green} {msg}")
                    .unwrap(),
            );
            spinner.set_message(format!(
                "Executing MCP tool '{tool}' on server '{server}'..."
            ));
            spinner.enable_steady_tick(Duration::from_millis(80));

            let res = mcp::call(&server, &tool, serde_json::from_str(&arguments)?);
            spinner.finish_and_clear();
            println!("{}", serde_json::to_string_pretty(&res?)?);
        }
        McpCommand::Registry { command } => match command {
            None | Some(RegistryCommand::List) => {
                println!("\n{BLUE}MCP server catalog:{RESET}");
                for e in mcp::registry() {
                    let needs = if e.required_env.is_empty() {
                        String::new()
                    } else {
                        format!(
                            " {WARN}(needs: {}){RESET}",
                            e.required_env
                                .iter()
                                .map(|v| v.key.as_str())
                                .collect::<Vec<_>>()
                                .join(", ")
                        )
                    };
                    println!("  {MINT}{:<20}{RESET} {}{needs}", e.key, e.desc);
                }
                println!(
                    "\n{DIM}Add one: mint mcp registry add <key> [--arg <v>] [--env K=V] [--allow-all]{RESET}"
                );
            }
            Some(RegistryCommand::Add {
                key,
                name,
                args,
                env,
                allow_all,
            }) => {
                let saved = mcp::registry_add(&key, name.as_deref(), args, env, allow_all)?;
                println!("Added MCP server: {saved}");
                if allow_all {
                    println!("Allowed all tools for {saved}");
                }
            }
        },
    }
    Ok(())
}

pub fn handle_link(command: LinkCommand) -> Result<()> {
    match command {
        LinkCommand::Add {
            name,
            path,
            description,
        } => {
            add_linked_folder(&name, &path, description)?;
            println!("Linked folder: {name}");
        }
        LinkCommand::List => {
            let folders = list_linked_folders()?;
            println!("{}", serde_json::to_string_pretty(&folders)?);
        }
        LinkCommand::Remove { name } => {
            println!(
                "{}",
                if remove_linked_folder(&name)? {
                    "removed"
                } else {
                    "not found"
                }
            )
        }
    }
    Ok(())
}

pub fn handle_hooks(command: HooksCommand) -> Result<()> {
    match command {
        HooksCommand::Add {
            event,
            matcher,
            command,
            timeout,
        } => {
            let event = mint_core::HookEvent::parse(&event)?;
            hooks::add(event, &matcher, &command, timeout)?;
            println!("Added {} hook for '{}'", event.as_str(), matcher);
        }
        HooksCommand::List => {
            let entries = hooks::list()?;
            if entries.is_empty() {
                println!("\n{BLUE}No hooks configured.{RESET}");
            } else {
                println!("\n{BLUE}Hooks:{RESET}");
                for (index, hook) in entries.iter().enumerate() {
                    println!(
                        "  [{}] {} matcher='{}' timeout={}s command={}",
                        index,
                        hook.event.as_str(),
                        hook.matcher,
                        hook.timeout_secs,
                        hook.command
                    );
                }
            }
        }
        HooksCommand::Remove { index } => {
            println!(
                "{}",
                if hooks::remove(index)? {
                    "removed"
                } else {
                    "not found"
                }
            )
        }
        HooksCommand::Clear => {
            hooks::clear()?;
            println!("cleared");
        }
    }
    Ok(())
}

pub async fn handle_gmail(command: GmailCommand) -> Result<()> {
    match command {
        GmailCommand::Auth { no_open, port } => gmail::auth(no_open, port).await,
    }
}

pub async fn handle_plugins(command: Option<plugins_cli::PluginsSubcommand>) -> Result<()> {
    plugins_cli::run_plugins_command(command).await
}

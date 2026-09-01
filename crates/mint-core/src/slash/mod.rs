//! UI-agnostic slash-command engine, shared by the CLI
//! (`crates/mint-cli/src/interactive/slash_commands.rs`), the web API server
//! (`crate::api_server::routes::slash`), and the desktop Tauri host
//! (`src-tauri/src/lib.rs::run_slash_command`).
//!
//! `execute` is a pure function of its `input` string plus the `MintConfig` it
//! is handed: it mutates the config in place for config commands and reports
//! what it changed through `SlashEffect`s, but never persists — the caller
//! writes `save_config` and refreshes its own UI. Interactive commands return a
//! `NeedsChoice` descriptor; the caller renders its own picker and re-invokes
//! `execute` with the chosen value appended to `input`. No engine-side session
//! state.
//!
//! Rollout is incremental: commands not yet migrated (and every CLI-only one)
//! return `NotHandled`, and each surface keeps its previous per-command path as
//! a fallback.

use crate::MintConfig;
use std::path::{Path, PathBuf};

/// The instruction `/init` hands to the code agent (via
/// [`SlashResponse::ForwardToAgent`]). Mirrors Claude Code's `/init`, but
/// targets `AGENTS.md` — the file every Mint surface already loads as
/// workspace rules (see `skills::load_agent_rules_file`).
pub const INIT_AGENTS_MD_PROMPT: &str = "\
Analyze this codebase and create an AGENTS.md file at the workspace root (or update it if one already exists).

AGENTS.md is loaded into every future Mint Agent session for this project, so it must give a fresh agent the context it needs without re-reading the whole tree.

Include:
- The exact build, run, lint, and test commands this repo uses.
- The high-level architecture — the big picture that only becomes clear after reading several files: the main components/crates/packages and how they fit together, key data flows, and where the entry points are.
- Project-specific conventions a contributor must follow (naming, layout, patterns) that aren't obvious from a single file.
- Non-obvious gotchas, constraints, or \"don't do X\" rules.

Rules:
- If AGENTS.md, .cursorrules, .github/copilot-instructions.md, or a similar rules file already exists, fold its still-relevant content in rather than discarding it.
- Be concise — bullet points over prose. Skip anything obvious from reading one file, and don't invent conventions that aren't actually in the code.
- Write only the file, then briefly confirm what you wrote.";

pub mod catalog;
pub mod models;
mod render;

use render::{local_time, md_heading, md_list, md_table};

#[derive(serde::Deserialize)]
pub struct SlashRequest {
    pub input: String,
    /// Active workspace directory; defaults to the process cwd.
    #[serde(default)]
    pub cwd: Option<String>,
    /// Calling surface: `"cli"`, `"web"`, or `"desktop"`. Only affects which
    /// commands `/help` lists; defaults to `"cli"`.
    #[serde(default)]
    pub surface: Option<String>,
}

impl SlashRequest {
    fn workspace(&self) -> PathBuf {
        self.cwd
            .as_deref()
            .filter(|s| !s.trim().is_empty())
            .map(PathBuf::from)
            .unwrap_or_else(|| std::env::current_dir().unwrap_or_default())
    }

    fn is_cli(&self) -> bool {
        self.surface.as_deref().unwrap_or("cli") == "cli"
    }
}

#[derive(serde::Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum SlashResponse {
    /// Nothing more to do; show this Markdown (may be empty).
    Message { markdown: String },
    /// Like `Message`, plus side effects the caller must react to (persist
    /// config, clear history, refresh status).
    Applied {
        markdown: String,
        effects: Vec<SlashEffect>,
    },
    /// The caller must collect a choice and re-invoke `execute` with
    /// `format!("{command} {value}")`.
    NeedsChoice {
        command: String,
        title: String,
        options: Vec<SlashChoice>,
    },
    /// Hand this string to the agent loop. `agent_mode` asks the GUI to switch
    /// into code-agent mode first (the CLI always runs forwarded input through
    /// the code agent regardless).
    ForwardToAgent { prompt: String, agent_mode: bool },
    /// GUI: switch to this view. CLI: print `markdown` as a hint.
    Navigate {
        target: SlashNavTarget,
        markdown: String,
    },
    /// CLI only — leave the interactive loop.
    Exit,
    /// Not a slash command this engine handles (yet); the caller falls back.
    NotHandled,
}

#[derive(serde::Serialize)]
pub struct SlashChoice {
    pub label: String,
    pub value: String,
}

#[derive(serde::Serialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum SlashEffect {
    /// `config` was mutated in place — caller must `save_config`.
    ConfigChanged,
    /// Active provider/model changed (also implies `ConfigChanged`).
    ProviderChanged {
        display: String,
    },
    /// `/cd` resolved to a new workspace.
    WorkspaceChanged {
        path: String,
    },
    /// `/clear` — caller wipes the current conversation.
    HistoryCleared,
    FastModeChanged {
        enabled: bool,
    },
    MultiAgentChanged {
        enabled: bool,
    },
}

#[derive(serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SlashNavTarget {
    Cron,
    LinkedFolders,
    Skills,
    Plugins,
    Mcp,
    Veo,
}

fn message(md: impl Into<String>) -> SlashResponse {
    SlashResponse::Message {
        markdown: md.into(),
    }
}

fn error(msg: impl std::fmt::Display) -> SlashResponse {
    SlashResponse::Message {
        markdown: format!("⚠️ {msg}"),
    }
}

/// Split `"/token rest..."` into `("/token" lowercased, "rest" trimmed)`.
fn split_input(input: &str) -> Option<(String, &str)> {
    let trimmed = input.trim();
    if !trimmed.starts_with('/') {
        return None;
    }
    match trimmed.split_once(char::is_whitespace) {
        Some((tok, rest)) => Some((tok.to_ascii_lowercase(), rest.trim())),
        None => Some((trimmed.to_ascii_lowercase(), "")),
    }
}

/// Split `"sub args..."` into `("sub" lowercased, "args" trimmed)`.
fn split_sub(rest: &str) -> (String, &str) {
    match rest.split_once(char::is_whitespace) {
        Some((s, a)) => (s.to_ascii_lowercase(), a.trim()),
        None => (rest.to_ascii_lowercase(), ""),
    }
}

pub fn execute(req: &SlashRequest, config: &mut MintConfig) -> SlashResponse {
    let Some((token, rest)) = split_input(&req.input) else {
        return SlashResponse::NotHandled;
    };

    match token.as_str() {
        "/help" => cmd_help(req.is_cli()),
        "/init" => SlashResponse::ForwardToAgent {
            prompt: INIT_AGENTS_MD_PROMPT.to_string(),
            agent_mode: true,
        },
        "/release-notes" => message(
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../Release_Note.md"
            ))
            .trim(),
        ),
        "/clear" => SlashResponse::Applied {
            markdown: "🧹 Conversation history cleared.".into(),
            effects: vec![SlashEffect::HistoryCleared],
        },

        "/autoskill" => cmd_bool_toggle(
            rest,
            "/autoskill",
            "Auto Skill Writing",
            config.auto_skill_writing,
            |cfg, v| cfg.auto_skill_writing = v,
            config,
        ),
        "/autorecall" => cmd_bool_toggle(
            rest,
            "/autorecall",
            "Memory Recall",
            config.memory_recall,
            |cfg, v| cfg.memory_recall = v,
            config,
        ),
        "/autofacts" => cmd_bool_toggle(
            rest,
            "/autofacts",
            "Auto Fact Extraction",
            config.auto_fact_extraction,
            |cfg, v| cfg.auto_fact_extraction = v,
            config,
        ),
        "/factrecall" => cmd_bool_toggle(
            rest,
            "/factrecall",
            "Semantic Fact Recall",
            config.semantic_fact_recall,
            |cfg, v| cfg.semantic_fact_recall = v,
            config,
        ),
        "/multi-agent" => {
            let current = config.enable_agent_collaboration;
            match parse_on_off(rest) {
                None if rest.is_empty() => cmd_multi_agent_status(config),
                None => error("Usage: /multi-agent [on|off]"),
                Some(v) => {
                    config.enable_agent_collaboration = v;
                    SlashResponse::Applied {
                        markdown: format!(
                            "🤖 Multi-Agent Collaboration **{}**{}.",
                            if v { "ON" } else { "OFF" },
                            if v == current { " (unchanged)" } else { "" }
                        ),
                        effects: vec![
                            SlashEffect::ConfigChanged,
                            SlashEffect::MultiAgentChanged { enabled: v },
                        ],
                    }
                }
            }
        }
        "/fast" => match parse_on_off(rest) {
            None if rest.is_empty() => needs_on_off("/fast", "Fast Mode (hide thinking)"),
            None => error("Usage: /fast [on|off]"),
            Some(v) => {
                config
                    .extra
                    .insert("fastMode".into(), serde_json::Value::Bool(v));
                SlashResponse::Applied {
                    markdown: format!("⚡ Fast Mode **{}**.", if v { "ON" } else { "OFF" }),
                    effects: vec![
                        SlashEffect::ConfigChanged,
                        SlashEffect::FastModeChanged { enabled: v },
                    ],
                }
            }
        },

        "/models" => cmd_models(rest, config),
        // `/searchProvider` is a documented camelCase alias (see UNDOCUMENTED_ALIASES).
        "/search-provider" | "/searchprovider" => cmd_extra_provider(
            "/search-provider",
            rest,
            "searchProvider",
            "web search",
            &["google", "brave", "searxng"],
            config,
        ),
        "/video-provider" => cmd_extra_provider(
            "/video-provider",
            rest,
            "videoGenProvider",
            "video generation",
            &["veo"],
            config,
        ),
        "/image-provider" => {
            if rest.is_empty() {
                return message(
                    "🎨 **Image providers:** nanobanana, dalle, stability, ideogram, replicate.\nUsage: `/image-provider <name>`",
                );
            }
            config.image_gen_provider = rest.to_string();
            SlashResponse::Applied {
                markdown: format!("🎨 Default image provider set to `{rest}`."),
                effects: vec![SlashEffect::ConfigChanged],
            }
        }

        "/cron" => cmd_cron(rest, &req.workspace()),
        "/link" => cmd_link(rest),
        "/subagent" => cmd_subagent(rest, &req.workspace()),
        "/mcp" => cmd_mcp(rest, config),

        "/cd" => cmd_cd(rest),
        "/stats" => cmd_stats(config, &req.workspace()),
        "/memory" => cmd_memory(rest),
        "/remember" => cmd_remember(rest, &req.workspace()),
        "/plugins" | "/plugin" => {
            if rest.is_empty() {
                SlashResponse::Navigate {
                    target: SlashNavTarget::Plugins,
                    markdown: "🔌 Opened Plugins.".into(),
                }
            } else {
                cmd_plugin(rest, config)
            }
        }
        "/skill" | "/learn" => SlashResponse::Navigate {
            target: SlashNavTarget::Skills,
            markdown: "📚 Opened Skills — import there or drop files in `.agents/skills/`.".into(),
        },
        "/code" => {
            if rest.is_empty() {
                error("Usage: /code <task>")
            } else {
                SlashResponse::ForwardToAgent {
                    prompt: rest.to_string(),
                    agent_mode: true,
                }
            }
        }
        "/generate-image" | "/gen-image" => {
            if rest.is_empty() {
                error(format!("Usage: `{token} <prompt>`"))
            } else {
                SlashResponse::ForwardToAgent {
                    prompt: format!("Generate an image of {rest}"),
                    agent_mode: false,
                }
            }
        }
        "/edit-image" => {
            if rest.is_empty() {
                error("Usage: /edit-image <instruction> (attach an image first)")
            } else {
                SlashResponse::ForwardToAgent {
                    prompt: format!("Edit the attached image: {rest}"),
                    agent_mode: false,
                }
            }
        }

        // Not migrated yet, or CLI-only — caller falls back.
        _ => SlashResponse::NotHandled,
    }
}

fn cmd_help(cli: bool) -> SlashResponse {
    let rows: Vec<Vec<String>> = catalog::SLASH_COMMANDS
        .iter()
        .filter(|c| {
            if cli {
                c.on_cli()
            } else {
                c.surfaces.iter().any(|s| s == "web" || s == "desktop")
            }
        })
        .map(|c| {
            vec![
                format!("`{}`", c.token),
                c.description.clone(),
                if c.usage.is_empty() {
                    String::new()
                } else {
                    format!("`{}`", c.usage)
                },
            ]
        })
        .collect();
    let mut md = md_heading("⚡ Slash Commands");
    md.push_str(&md_table(&["Command", "Description", "Usage"], &rows));
    message(md)
}

fn parse_on_off(s: &str) -> Option<bool> {
    match s.trim().to_ascii_lowercase().as_str() {
        "on" | "true" | "enable" => Some(true),
        "off" | "false" | "disable" => Some(false),
        _ => None,
    }
}

fn needs_on_off(command: &str, title: &str) -> SlashResponse {
    SlashResponse::NeedsChoice {
        command: command.to_string(),
        title: format!("{title} — on or off?"),
        options: vec![
            SlashChoice {
                label: "on".into(),
                value: "on".into(),
            },
            SlashChoice {
                label: "off".into(),
                value: "off".into(),
            },
        ],
    }
}

fn cmd_bool_toggle(
    rest: &str,
    command: &str,
    label: &str,
    current: bool,
    set: fn(&mut MintConfig, bool),
    config: &mut MintConfig,
) -> SlashResponse {
    match parse_on_off(rest) {
        None if rest.is_empty() => needs_on_off(command, label),
        None => error(format!("Usage: {command} [on|off]")),
        Some(v) => {
            set(config, v);
            SlashResponse::Applied {
                markdown: format!(
                    "✅ {label} **{}**{}.",
                    if v { "ON" } else { "OFF" },
                    if v == current { " (unchanged)" } else { "" }
                ),
                effects: vec![SlashEffect::ConfigChanged],
            }
        }
    }
}

fn cmd_models(rest: &str, config: &mut MintConfig) -> SlashResponse {
    if rest.is_empty() {
        let providers = config.available_providers();
        if providers.is_empty() {
            return error("No providers are configured — add an API key in Settings first.");
        }
        return SlashResponse::NeedsChoice {
            command: "/models".into(),
            title: "Select AI provider".into(),
            options: providers
                .into_iter()
                .map(|p| SlashChoice {
                    label: p.clone(),
                    value: p,
                })
                .collect(),
        };
    }
    // `/models <provider>` (optionally `<provider> <model>` or `<provider>/<model>`).
    let (provider, model) = match rest.split_once(['/', ' ']) {
        Some((p, m)) => (p.trim(), Some(m.trim()).filter(|m| !m.is_empty())),
        None => (rest.trim(), None),
    };

    // Provider given but no model yet — offer the model picker when we know one.
    if model.is_none() {
        let options = models::model_options_for_provider(config, provider);
        if !options.is_empty() {
            return SlashResponse::NeedsChoice {
                command: format!("/models {provider}"),
                title: format!("Select {provider} model"),
                options: options
                    .into_iter()
                    .map(|m| SlashChoice {
                        label: m.clone(),
                        value: m,
                    })
                    .collect(),
            };
        }
    }

    let was = (
        config.ai_provider.clone(),
        config.active_model().to_string(),
    );

    // Mutate the config in place and let the caller persist (via the
    // `ConfigChanged` effect) — deliberately NOT `config.set_active_model`,
    // which writes `save_config` to the real config path as a hidden side
    // effect (that made an engine unit test overwrite a user's config once).
    set_active_provider_model(config, provider, model);

    let display = format!(
        "{} • {}",
        models::provider_display_name(config, &config.ai_provider),
        config.active_model()
    );

    // Log a `provider_change` row into the shared "cli" conversation when the
    // active model actually changed, so the other surface picks it up (CLI via
    // live_sync, GUI via its conversation refresh) and renders the same chip —
    // this is the cross-surface sync the old `set_active_model` used to do.
    // Skipped under `cfg!(test)`: `MemoryStore::open_default()` resolves to the
    // real user data dir with no test-scoped override, so without this guard
    // every `cmd_models` test run leaves a junk row in the developer's own chat
    // (same reasoning as `cron::store`'s `ensure_named_chat_session` guard).
    if !cfg!(test)
        && was
            != (
                config.ai_provider.clone(),
                config.active_model().to_string(),
            )
    {
        if let Ok(memory) = crate::MemoryStore::open_default() {
            let _ = memory.add_interaction_for_chat_with_fallback(
                crate::CHAT_CLI_ID,
                &display,
                "",
                "system",
                "provider_change",
                None,
            );
        }
    }

    SlashResponse::Applied {
        // No Markdown body: the `provider_change` chip is the feedback on both
        // surfaces (see the `ProviderChanged` handling in each host).
        markdown: String::new(),
        effects: vec![
            SlashEffect::ConfigChanged,
            SlashEffect::ProviderChanged { display },
        ],
    }
}

/// Set `config.ai_provider` and the provider-specific model field. Pure
/// mutation, no persistence — the non-saving half of `MintConfig::set_active_model`.
fn set_active_provider_model(config: &mut MintConfig, provider: &str, model: Option<&str>) {
    config.ai_provider = provider.to_string();
    if let Some(m) = model.map(str::trim).filter(|m| !m.is_empty()) {
        match provider {
            "anthropic" => config.anthropic_model = m.to_string(),
            "openai" => config.openai_model = m.to_string(),
            "openrouter" => config.openrouter_model = m.to_string(),
            "deepseek" => config.deepseek_model = m.to_string(),
            "huggingface" => config.hf_model = m.to_string(),
            "local_openai" => config.local_model_name = m.to_string(),
            "ollama" => config.ollama_model = m.to_string(),
            "gemini" => config.gemini_model = m.to_string(),
            _ => {}
        }
    }
}

/// `/multi-agent` with no argument: show the current status + configured agents.
fn cmd_multi_agent_status(config: &MintConfig) -> SlashResponse {
    let mut md = md_heading("🤖 Multi-Agent Collaboration");
    md.push_str(&format!(
        "Global collaboration: **{}**\n\n",
        if config.enable_agent_collaboration {
            "on"
        } else {
            "off"
        }
    ));
    if config.agents.is_empty() {
        md.push_str("_No agents configured._\n");
    } else {
        let items: Vec<String> = config
            .agents
            .iter()
            .map(|a| {
                format!(
                    "**{}** {} — {} · {}",
                    a.name,
                    if a.enabled { "on" } else { "off" },
                    a.provider,
                    a.model
                )
            })
            .collect();
        md.push_str(&md_list(&items));
    }
    md.push_str("\n\n_Usage: `/multi-agent [on|off]`_");
    message(md)
}

fn cmd_cd(rest: &str) -> SlashResponse {
    if rest.is_empty() {
        return error("Usage: /cd <path>");
    }
    let dir = Path::new(rest);
    if !dir.is_dir() {
        return error(format!("Directory not found: {rest}"));
    }
    let path = dir
        .canonicalize()
        .unwrap_or_else(|_| dir.to_path_buf())
        .to_string_lossy()
        .into_owned();
    SlashResponse::Applied {
        markdown: format!("📁 Workspace: `{path}`"),
        effects: vec![SlashEffect::WorkspaceChanged { path }],
    }
}

fn cmd_stats(config: &MintConfig, workspace: &Path) -> SlashResponse {
    let cron_count = crate::CronStore::open_default()
        .and_then(|s| s.list())
        .map(|j| j.len())
        .unwrap_or(0);
    let link_count = crate::list_linked_folders().map(|f| f.len()).unwrap_or(0);
    let fast = config
        .extra
        .get("fastMode")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let mut md = md_heading("📊 Session Stats");
    md.push_str(&md_list(&[
        format!("Provider: `{}`", config.ai_provider),
        format!("Model: `{}`", config.active_model()),
        format!("Workspace: `{}`", workspace.display()),
        format!("Fast mode: {}", if fast { "on" } else { "off" }),
        format!(
            "Multi-agent: {}",
            if config.enable_agent_collaboration {
                "on"
            } else {
                "off"
            }
        ),
        format!("Cron jobs: {cron_count}"),
        format!("Linked folders: {link_count}"),
    ]));
    message(md)
}

fn cmd_memory(rest: &str) -> SlashResponse {
    let store = match crate::MemoryStore::open_default() {
        Ok(s) => s,
        Err(e) => return error(e),
    };
    let (sub, args) = split_sub(rest);
    match sub.as_str() {
        "get" if !args.is_empty() => match store.get_profile(args) {
            Ok(Some(value)) => message(format!("`{args}` = {value}")),
            Ok(None) => message(format!("`{args}` is not set.")),
            Err(e) => error(e),
        },
        "set" => {
            let (key, value) = split_sub(args);
            if key.is_empty() {
                return error("Usage: /memory set <key> <value>");
            }
            match store.set_profile(&key, value) {
                Ok(()) => message(format!("🧠 Stored `{key}`.")),
                Err(e) => error(e),
            }
        }
        "clear" => SlashResponse::Applied {
            markdown: "🧠 Cleared stored interactions for this conversation.".into(),
            effects: vec![SlashEffect::HistoryCleared],
        },
        "facts" => match store.list_facts(50) {
            Ok(facts) if facts.is_empty() => {
                message("🧠 No stored facts yet. Add one with `/remember <text>`.")
            }
            Ok(facts) => {
                let rows = facts
                    .iter()
                    .map(|f| vec![f.id.to_string(), fact_owner_label(f), f.body.clone()])
                    .collect::<Vec<_>>();
                message(md_table(&["id", "owner", "fact"], &rows))
            }
            Err(e) => error(e),
        },
        "forget" if !args.is_empty() => match store.forget_fact(args) {
            Ok(0) => message(format!("Nothing matched `{args}`.")),
            Ok(n) => message(format!("🧠 Forgot {n} fact(s).")),
            Err(e) => error(e),
        },
        "forget" => error("Usage: /memory forget <id-or-text>"),
        "promote" if !args.is_empty() => match args.parse::<i64>() {
            Ok(id) => match store.promote_fact(id) {
                Ok(true) => message(format!("🧠 Promoted fact {id} to shared memory.")),
                Ok(false) => message(format!(
                    "Fact {id} is not a subagent-scoped fact (nothing to promote)."
                )),
                Err(e) => error(e),
            },
            Err(_) => error("Usage: /memory promote <id>"),
        },
        "promote" => error("Usage: /memory promote <id>"),
        "" | "list" => message(
            "🧠 **Long-term memory** — `/remember <text>` to add a fact · `/memory facts` to list · `/memory forget <id>` · `/memory promote <id>` · `/memory get <key>` · `/memory set <key> <value>` · `/memory clear`.",
        ),
        _ => error(
            "Usage: /memory list | facts | forget <id> | promote <id> | clear | get <key> | set <key> <value>",
        ),
    }
}

/// Column shown by `/memory facts`: `via <subagent>` for a quarantined fact,
/// else `project` / `global` by scope.
fn fact_owner_label(fact: &crate::Fact) -> String {
    match &fact.agent_id {
        Some(name) => format!("via {name}"),
        None if fact.scope == "project" => "project".into(),
        None => "global".into(),
    }
}

fn cmd_remember(rest: &str, workspace: &Path) -> SlashResponse {
    let (first, tail) = split_sub(rest);
    let (scope, project_path, body) = if first == "here" {
        (
            "project",
            Some(workspace.to_string_lossy().into_owned()),
            tail,
        )
    } else {
        ("user", None, rest.trim())
    };
    if body.is_empty() {
        return error("Usage: /remember [here] <text>");
    }
    let store = match crate::MemoryStore::open_default() {
        Ok(s) => s,
        Err(e) => return error(e),
    };
    match store.add_fact(scope, project_path.as_deref(), body, None, None) {
        Ok(Some(_)) => message(if scope == "project" {
            "🧠 Remembered (this project).".to_string()
        } else {
            "🧠 Remembered.".to_string()
        }),
        Ok(None) => message("🧠 Already remembered."),
        Err(e) => error(e),
    }
}

fn cmd_extra_provider(
    command: &str,
    rest: &str,
    key: &str,
    label: &str,
    known: &[&str],
    config: &mut MintConfig,
) -> SlashResponse {
    if rest.is_empty() {
        return message(format!(
            "🔎 **{label} providers:** {}.\nUsage: `{command} <name>`",
            known.join(", "),
        ));
    }
    let value = rest.to_ascii_lowercase();
    config
        .extra
        .insert(key.to_string(), serde_json::Value::String(value.clone()));
    SlashResponse::Applied {
        markdown: format!("✅ Default {label} provider set to `{value}`."),
        effects: vec![SlashEffect::ConfigChanged],
    }
}

fn cmd_cron(rest: &str, workspace: &Path) -> SlashResponse {
    let store = match crate::CronStore::open_default() {
        Ok(s) => s,
        Err(e) => return error(e),
    };
    let (sub, args) = split_sub(rest);
    match sub.as_str() {
        "" | "list" => match store.list() {
            Ok(jobs) if jobs.is_empty() => message(
                "⏰ No scheduled tasks. Add one with `/cron add <name> | <schedule> | <task>`.",
            ),
            Ok(jobs) => {
                let rows = jobs
                    .iter()
                    .map(|j| {
                        vec![
                            format!("`{}`", j.id),
                            j.name.clone(),
                            if j.enabled { "on".into() } else { "off".into() },
                            local_time(&j.next_run),
                        ]
                    })
                    .collect::<Vec<_>>();
                let mut md = md_heading("⏰ Scheduled Tasks");
                md.push_str(&md_table(&["ID", "Name", "State", "Next run"], &rows));
                message(md)
            }
            Err(e) => error(e),
        },
        "add" => {
            let fields: Vec<&str> = args.splitn(4, '|').map(str::trim).collect();
            let (name, schedule, task, tz) = match fields.as_slice() {
                [n, s, t] => (*n, *s, *t, None),
                [n, s, t, z] => (*n, *s, *t, Some(*z)),
                _ => return error("Usage: /cron add <name> | <schedule> | <task> | [timezone]"),
            };
            if name.is_empty() || task.is_empty() {
                return error("Usage: /cron add <name> | <schedule> | <task> | [timezone]");
            }
            let schedule = match tz {
                Some(tz) if !tz.is_empty() => {
                    match crate::localize_schedule(schedule, tz, chrono::Utc::now()) {
                        Ok(s) => s,
                        Err(e) => return error(e),
                    }
                }
                _ => schedule.to_string(),
            };
            match store.add(name, schedule, task, workspace.to_path_buf()) {
                Ok(job) => SlashResponse::Applied {
                    markdown: format!(
                        "⏰ Created cron job `{}` — next run: {}",
                        job.id,
                        local_time(&job.next_run)
                    ),
                    effects: vec![],
                },
                Err(e) => error(e),
            }
        }
        "remove" | "rm" if !args.is_empty() => match store.remove(args) {
            Ok(true) => message(format!("🗑️ Removed `{args}`.")),
            Ok(false) => error(format!("No cron job with id `{args}`.")),
            Err(e) => error(e),
        },
        "enable" | "disable" if !args.is_empty() => {
            match store.set_enabled(args, sub == "enable") {
                Ok(Some(_)) => message(format!("✅ {sub}d `{args}`.")),
                Ok(None) => error(format!("No cron job with id `{args}`.")),
                Err(e) => error(e),
            }
        }
        _ => error(
            "Usage: /cron [list] | add <name> | <schedule> | <task> | [tz] | remove <id> | enable <id> | disable <id>",
        ),
    }
}

fn cmd_link(rest: &str) -> SlashResponse {
    let (sub, args) = split_sub(rest);
    match sub.as_str() {
        "" | "list" => match crate::list_linked_folders() {
            Ok(folders) if folders.is_empty() => {
                message("🔗 No linked folders. Add one with `/link add <name> | <path> | <desc>`.")
            }
            Ok(folders) => {
                let items = folders
                    .values()
                    .map(|f| {
                        format!(
                            "**{}** — `{}`{}",
                            f.name,
                            f.path.display(),
                            f.description
                                .as_deref()
                                .filter(|d| !d.is_empty())
                                .map(|d| format!(" — {d}"))
                                .unwrap_or_default()
                        )
                    })
                    .collect::<Vec<_>>();
                let mut md = md_heading("🔗 Linked Folders");
                md.push_str(&md_list(&items));
                message(md)
            }
            Err(e) => error(e),
        },
        "add" => {
            let fields: Vec<&str> = args.splitn(3, '|').map(str::trim).collect();
            let (name, path, desc) = match fields.as_slice() {
                [n, p] => (*n, *p, None),
                [n, p, d] => (*n, *p, Some(d.to_string()).filter(|d| !d.is_empty())),
                _ => return error("Usage: /link add <name> | <path> | <description>"),
            };
            if name.is_empty() || path.is_empty() {
                return error("Usage: /link add <name> | <path> | <description>");
            }
            match crate::add_linked_folder(name, Path::new(path), desc) {
                Ok(()) => message(format!("🔗 Linked folder `{name}`.")),
                Err(e) => error(e),
            }
        }
        "remove" | "rm" if !args.is_empty() => match crate::remove_linked_folder(args) {
            Ok(true) => message(format!("🗑️ Removed `{args}`.")),
            Ok(false) => error(format!("No linked folder named `{args}`.")),
            Err(e) => error(e),
        },
        _ => error("Usage: /link [list] | add <name> | <path> | <desc> | remove <name>"),
    }
}

fn cmd_subagent(rest: &str, workspace: &Path) -> SlashResponse {
    let (sub, _args) = split_sub(rest);
    match sub.as_str() {
        "" | "list" => {
            let subs = crate::list_subagents(Some(workspace));
            if subs.is_empty() {
                return message("🤖 No subagents configured.");
            }
            let items = subs
                .iter()
                .map(|s| {
                    let src = if s.builtin {
                        "built-in"
                    } else if s.source_path.contains(".agents/subagents") {
                        "workspace"
                    } else {
                        "global"
                    };
                    format!("**{}** _{}_ — {}", s.name, src, s.description)
                })
                .collect::<Vec<_>>();
            let mut md = md_heading("🤖 Subagents");
            md.push_str(&md_list(&items));
            message(md)
        }
        _ => SlashResponse::Navigate {
            target: SlashNavTarget::Skills,
            markdown: "Manage subagents in Settings.".into(),
        },
    }
}

/// Wrap a config mutation the host must persist.
fn applied(md: impl Into<String>) -> SlashResponse {
    SlashResponse::Applied {
        markdown: md.into(),
        effects: vec![SlashEffect::ConfigChanged],
    }
}

const MCP_USAGE: &str = "Usage: /mcp [list] | add <name> <cmd> [args…] | remove <name> | \
    enable|disable <name> | edit <name> command|args|icon <value> | \
    allow|disallow <server> <tool> | reauth <server> | clear";

fn cmd_mcp(rest: &str, config: &mut MintConfig) -> SlashResponse {
    let (sub, args) = split_sub(rest);
    match sub.as_str() {
        "" | "list" => match crate::list_mcp_servers() {
            Ok(servers) if servers.is_empty() => {
                message("🔌 No MCP servers configured. Add one with `/mcp add`, or in Settings.")
            }
            Ok(servers) => {
                let items = servers
                    .iter()
                    .map(|(name, server)| {
                        if server.disabled {
                            format!("`{name}` _(disabled)_")
                        } else {
                            format!("`{name}`")
                        }
                    })
                    .collect::<Vec<_>>();
                let mut md = md_heading("🔌 MCP Servers");
                md.push_str(&md_list(&items));
                message(md)
            }
            Err(e) => error(e),
        },

        "add" => {
            // A trailing `--allow-all` anywhere in the tail opts every tool in.
            let mut tokens: Vec<&str> = args.split_whitespace().collect();
            let allow_all = tokens.iter().any(|t| *t == "--allow-all");
            tokens.retain(|t| *t != "--allow-all");
            let mut parts = tokens.into_iter();
            match (parts.next(), parts.next()) {
                (Some(name), Some(command)) => {
                    let server = crate::McpServer {
                        command: command.to_string(),
                        args: parts.map(str::to_string).collect(),
                        env: Default::default(),
                        icon: None,
                        disabled: false,
                    };
                    match crate::upsert_server_in(config, name, server) {
                        Ok(()) => {
                            if allow_all {
                                crate::allow_tool_in(config, name, "*");
                            }
                            applied(format!(
                                "🔌 Added MCP server `{name}`{}.",
                                if allow_all {
                                    " (all tools allowed)"
                                } else {
                                    ""
                                }
                            ))
                        }
                        Err(e) => error(e),
                    }
                }
                _ => error("Usage: /mcp add <name> <command> [args…] [--allow-all]"),
            }
        }

        "remove" if !args.is_empty() => match crate::remove_server_in(config, args) {
            Ok(true) => applied(format!("🔌 Removed MCP server `{args}`.")),
            Ok(false) => error(format!("No MCP server named `{args}`.")),
            Err(e) => error(e),
        },

        "clear" => match crate::clear_servers_in(config) {
            Ok(()) => applied("🔌 Removed all MCP servers."),
            Err(e) => error(e),
        },

        "enable" | "disable" if !args.is_empty() => {
            let disable = sub == "disable";
            match crate::set_server_disabled_in(config, args, disable) {
                Ok(true) => applied(format!(
                    "🔌 {} `{args}`.",
                    if disable { "Disabled" } else { "Enabled" }
                )),
                Ok(false) => error(format!("No MCP server named `{args}`.")),
                Err(e) => error(e),
            }
        }

        "edit" => {
            let mut parts = args.splitn(3, char::is_whitespace);
            let (Some(name), Some(field)) = (parts.next(), parts.next()) else {
                return error("Usage: /mcp edit <name> command|args|icon <value>");
            };
            let value = parts.next().unwrap_or("").trim();
            let edit = match field.to_ascii_lowercase().as_str() {
                "command" if !value.is_empty() => (Some(value.to_string()), None, None),
                "command" => return error("`/mcp edit <name> command <value>` needs a value"),
                "args" => (
                    None,
                    Some(value.split_whitespace().map(str::to_string).collect()),
                    None,
                ),
                "icon" => (
                    None,
                    None,
                    Some((!value.is_empty()).then(|| value.to_string())),
                ),
                _ => return error("Editable fields: command | args | icon"),
            };
            match crate::update_server_in(config, name, edit.0, edit.1, None, edit.2) {
                Ok(true) => applied(format!("🔌 Updated `{name}`.")),
                Ok(false) => error(format!("No MCP server named `{name}`.")),
                Err(e) => error(e),
            }
        }

        "allow" => {
            let mut parts = args.split_whitespace();
            match (parts.next(), parts.next()) {
                (Some(server), Some(tool)) => {
                    let added = crate::allow_tool_in(config, server, tool);
                    applied(if added {
                        format!("🔌 Allowed MCP tool `{server}/{tool}`.")
                    } else {
                        format!("🔌 `{server}/{tool}` was already allowed.")
                    })
                }
                _ => error("Usage: /mcp allow <server> <tool>"),
            }
        }

        "disallow" => {
            let mut parts = args.split_whitespace();
            match (parts.next(), parts.next()) {
                (Some(server), Some(tool)) => {
                    let removed = crate::disallow_tool_in(config, server, tool);
                    applied(if removed {
                        format!("🔌 Removed `{server}/{tool}` from the allowlist.")
                    } else {
                        format!("🔌 `{server}/{tool}` was not in the allowlist.")
                    })
                }
                _ => error("Usage: /mcp disallow <server> <tool>  (tool `*` clears the list)"),
            }
        }

        "reauth" if !args.is_empty() => match crate::reauth_mcp_server(args) {
            Ok(_) => message(format!("🔌 Re-ran OAuth for `{args}`.")),
            Err(e) => error(e),
        },

        _ => error(MCP_USAGE),
    }
}

fn cmd_plugin(rest: &str, config: &mut MintConfig) -> SlashResponse {
    let (sub, name) = split_sub(rest);
    match sub.as_str() {
        "enable" | "disable" if !name.is_empty() => {
            if !crate::native_plugins().iter().any(|p| p.name == name) {
                let known = crate::native_plugins()
                    .iter()
                    .map(|p| p.name)
                    .collect::<Vec<_>>()
                    .join(", ");
                return error(format!("Unknown native plugin `{name}`. Known: {known}"));
            }
            let enable = sub == "enable";
            crate::set_native_plugin_enabled_in(config, &name, enable);
            applied(format!(
                "🔌 {} native plugin `{name}`.",
                if enable { "Enabled" } else { "Disabled" }
            ))
        }
        _ => error("Usage: /plugin enable|disable <name>"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // These tests only ever pass a fresh `MintConfig::default()` into `execute`
    // and assert on the returned `SlashResponse` / the in-memory mutation. No
    // command in the engine persists to disk: `cmd_models` sets fields directly
    // (never `MintConfig::set_active_model`, which self-`save_config`s), and the
    // one chat-history write (`provider_change`) is `cfg!(test)`-guarded. So
    // there is deliberately no config-path fixture here to get wrong — an
    // earlier attempt at one (`XDG_CONFIG_HOME` override + a sentinel-file test)
    // had an ordering bug that overwrote the developer's real config twice.

    fn req(input: &str) -> SlashRequest {
        SlashRequest {
            input: input.to_string(),
            cwd: None,
            surface: None,
        }
    }

    fn req_surface(input: &str, surface: &str) -> SlashRequest {
        SlashRequest {
            input: input.to_string(),
            cwd: None,
            surface: Some(surface.to_string()),
        }
    }

    #[test]
    fn remember_without_text_shows_usage() {
        // The no-arg path returns before opening the memory store, so this
        // stays in line with the "no slash test persists to disk" rule above.
        let mut cfg = MintConfig::default();
        match execute(&req("/remember"), &mut cfg) {
            SlashResponse::Message { markdown } => {
                assert!(markdown.contains("Usage: /remember"));
            }
            other => panic!(
                "expected usage message, got {:?}",
                serde_json::to_value(other)
            ),
        }
    }

    #[test]
    fn unknown_and_cli_only_are_not_handled() {
        let mut cfg = MintConfig::default();
        assert!(matches!(
            execute(&req("/nonsense"), &mut cfg),
            SlashResponse::NotHandled
        ));
        assert!(matches!(
            execute(&req("/bg do a thing"), &mut cfg),
            SlashResponse::NotHandled
        ));
        assert!(matches!(
            execute(&req("hello not a slash"), &mut cfg),
            SlashResponse::NotHandled
        ));
    }

    #[test]
    fn autoskill_on_flips_config() {
        let mut cfg = MintConfig::default();
        cfg.auto_skill_writing = false;
        match execute(&req("/autoskill on"), &mut cfg) {
            SlashResponse::Applied { effects, .. } => {
                assert!(cfg.auto_skill_writing);
                assert!(effects.contains(&SlashEffect::ConfigChanged));
            }
            other => panic!("expected Applied, got {:?}", serde_json::to_value(other)),
        }
    }

    #[test]
    fn autofacts_off_flips_config() {
        let mut cfg = MintConfig::default();
        assert!(cfg.auto_fact_extraction);
        match execute(&req("/autofacts off"), &mut cfg) {
            SlashResponse::Applied { effects, .. } => {
                assert!(!cfg.auto_fact_extraction);
                assert!(effects.contains(&SlashEffect::ConfigChanged));
            }
            other => panic!("expected Applied, got {:?}", serde_json::to_value(other)),
        }
    }

    #[test]
    fn factrecall_off_flips_config() {
        let mut cfg = MintConfig::default();
        assert!(cfg.semantic_fact_recall);
        match execute(&req("/factrecall off"), &mut cfg) {
            SlashResponse::Applied { effects, .. } => {
                assert!(!cfg.semantic_fact_recall);
                assert!(effects.contains(&SlashEffect::ConfigChanged));
            }
            other => panic!("expected Applied, got {:?}", serde_json::to_value(other)),
        }
    }

    #[test]
    fn autoskill_no_arg_needs_choice() {
        let mut cfg = MintConfig::default();
        assert!(matches!(
            execute(&req("/autoskill"), &mut cfg),
            SlashResponse::NeedsChoice { .. }
        ));
    }

    #[test]
    fn fast_toggle_writes_extra() {
        let mut cfg = MintConfig::default();
        match execute(&req("/fast on"), &mut cfg) {
            SlashResponse::Applied { effects, .. } => {
                assert_eq!(
                    cfg.extra.get("fastMode"),
                    Some(&serde_json::Value::Bool(true))
                );
                assert!(effects.contains(&SlashEffect::FastModeChanged { enabled: true }));
            }
            _ => panic!("expected Applied"),
        }
    }

    #[test]
    fn clear_reports_history_cleared() {
        let mut cfg = MintConfig::default();
        match execute(&req("/clear"), &mut cfg) {
            SlashResponse::Applied { effects, .. } => {
                assert!(effects.contains(&SlashEffect::HistoryCleared));
            }
            _ => panic!("expected Applied"),
        }
    }

    #[test]
    fn models_provider_then_model_two_step() {
        let mut cfg = MintConfig::default();
        // provider given, no model -> model picker (gemini has static presets)
        match execute(&req("/models gemini"), &mut cfg) {
            SlashResponse::NeedsChoice {
                command, options, ..
            } => {
                assert_eq!(command, "/models gemini");
                assert!(!options.is_empty());
            }
            other => panic!(
                "expected NeedsChoice, got {:?}",
                serde_json::to_value(other)
            ),
        }
        // provider + model -> applied
        match execute(&req("/models gemini gemini-2.5-flash"), &mut cfg) {
            SlashResponse::Applied { effects, .. } => {
                assert_eq!(cfg.ai_provider, "gemini");
                assert_eq!(cfg.gemini_model, "gemini-2.5-flash");
                assert!(
                    effects
                        .iter()
                        .any(|e| matches!(e, SlashEffect::ProviderChanged { .. }))
                );
            }
            other => panic!("expected Applied, got {:?}", serde_json::to_value(other)),
        }
    }

    #[test]
    fn cd_reports_workspace_changed() {
        let mut cfg = MintConfig::default();
        let tmp = std::env::temp_dir();
        match execute(&req(&format!("/cd {}", tmp.display())), &mut cfg) {
            SlashResponse::Applied { effects, .. } => {
                assert!(
                    effects
                        .iter()
                        .any(|e| matches!(e, SlashEffect::WorkspaceChanged { .. }))
                );
            }
            other => panic!("expected Applied, got {:?}", serde_json::to_value(other)),
        }
        assert!(matches!(
            execute(&req("/cd /no/such/dir/xyz"), &mut cfg),
            SlashResponse::Message { .. }
        ));
    }

    #[test]
    fn mcp_allow_mutates_config() {
        let mut cfg = MintConfig::default();
        match execute(&req("/mcp allow srv toolA"), &mut cfg) {
            SlashResponse::Applied { effects, .. } => {
                assert!(effects.contains(&SlashEffect::ConfigChanged));
            }
            other => panic!("expected Applied, got {:?}", serde_json::to_value(other)),
        }
        let allowed = &cfg.extra["allowedMcpTools"]["srv"];
        assert_eq!(allowed[0], "toolA");
    }

    #[test]
    fn mcp_add_disable_edit_remove_round_trip() {
        let mut cfg = MintConfig::default();

        assert!(matches!(
            execute(&req("/mcp add demo sh -c cat"), &mut cfg),
            SlashResponse::Applied { .. }
        ));
        assert_eq!(cfg.extra["mcpServers"]["demo"]["command"], "sh");
        assert_eq!(cfg.extra["mcpServers"]["demo"]["args"][1], "cat");

        execute(&req("/mcp disable demo"), &mut cfg);
        assert_eq!(cfg.extra["mcpServers"]["demo"]["disabled"], true);
        execute(&req("/mcp enable demo"), &mut cfg);
        assert!(cfg.extra["mcpServers"]["demo"].get("disabled").is_none());

        execute(&req("/mcp edit demo icon 🧪"), &mut cfg);
        assert_eq!(cfg.extra["mcpServers"]["demo"]["icon"], "🧪");

        // `--allow-all` on add opts every tool in; plain add does not.
        execute(&req("/mcp add wide sh -c cat --allow-all"), &mut cfg);
        assert_eq!(cfg.extra["allowedMcpTools"]["wide"][0], "*");
        assert_eq!(cfg.extra["mcpServers"]["wide"]["args"][1], "cat");
        assert!(
            cfg.extra
                .get("allowedMcpTools")
                .and_then(|m| m.get("demo"))
                .is_none()
        );

        execute(&req("/mcp remove demo"), &mut cfg);
        assert!(
            cfg.extra["mcpServers"]
                .as_object()
                .unwrap()
                .get("demo")
                .is_none()
        );

        // Unknown server → not persisted, friendly message.
        assert!(matches!(
            execute(&req("/mcp disable ghost"), &mut cfg),
            SlashResponse::Message { .. }
        ));
    }

    #[test]
    fn plugin_enable_disable_flips_native_gate() {
        let mut cfg = MintConfig::default();

        match execute(&req("/plugin enable docker"), &mut cfg) {
            SlashResponse::Applied { effects, .. } => {
                assert!(effects.contains(&SlashEffect::ConfigChanged));
            }
            other => panic!("expected Applied, got {:?}", serde_json::to_value(other)),
        }
        assert!(crate::native_plugin_enabled(&cfg, "docker"));
        execute(&req("/plugin disable docker"), &mut cfg);
        assert!(!crate::native_plugin_enabled(&cfg, "docker"));

        // `/plugins` with no args still opens the GUI view.
        assert!(matches!(
            execute(&req("/plugins"), &mut cfg),
            SlashResponse::Navigate { .. }
        ));
        // Unknown plugin name is rejected without touching config.
        assert!(matches!(
            execute(&req("/plugin enable nope"), &mut cfg),
            SlashResponse::Message { .. }
        ));
    }

    #[test]
    fn code_forwards_in_agent_mode() {
        let mut cfg = MintConfig::default();
        match execute(&req("/code fix the parser"), &mut cfg) {
            SlashResponse::ForwardToAgent { prompt, agent_mode } => {
                assert_eq!(prompt, "fix the parser");
                assert!(agent_mode);
            }
            other => panic!(
                "expected ForwardToAgent, got {:?}",
                serde_json::to_value(other)
            ),
        }
    }

    #[test]
    fn responses_serialize_to_tagged_json() {
        let mut cfg = MintConfig::default();
        // `/fast on` -> Applied with a (previously tuple) effect variant.
        let json = serde_json::to_value(execute(&req("/fast on"), &mut cfg)).unwrap();
        assert_eq!(json["kind"], "applied");
        assert_eq!(json["effects"][1]["kind"], "fast_mode_changed");
        assert_eq!(json["effects"][1]["enabled"], true);
        // `/models` (no arg) -> NeedsChoice.
        let json = serde_json::to_value(execute(&req("/models"), &mut cfg)).unwrap();
        assert!(json["kind"] == "needs_choice" || json["kind"] == "message");
    }

    #[test]
    fn help_is_surface_aware() {
        let mut cfg = MintConfig::default();
        match execute(&req_surface("/help", "web"), &mut cfg) {
            SlashResponse::Message { markdown } => {
                assert!(markdown.contains("/cron"));
                assert!(!markdown.contains("`/bg`")); // cli-only, excluded from the gui list
            }
            _ => panic!("expected Message"),
        }
        match execute(&req_surface("/help", "cli"), &mut cfg) {
            SlashResponse::Message { markdown } => {
                assert!(markdown.contains("`/bg`")); // cli-only, present in the cli list
            }
            _ => panic!("expected Message"),
        }
    }
}

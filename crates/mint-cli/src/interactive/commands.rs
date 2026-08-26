//! Single source of truth for slash-command metadata shown in the interactive
//! chat — consumed by both the `/help` listing (`slash_commands.rs`) and the
//! `/` autocomplete dropdown (`input_box.rs`). Previously these were two
//! separate hand-maintained lists (plus the dispatcher `match` in
//! `slash_commands.rs::handle_slash_command`, a third), and they drifted:
//! `/edit-image`, `/gen-image`, `/shells`, and `/subagent` worked but were
//! missing from `/help` for a while before anyone noticed. See the
//! `dispatcher_tokens_are_documented` test in `slash_commands.rs`, which now
//! catches that class of bug at `cargo test` time.

pub struct SlashCommandSpec {
    /// Exact literal the autocomplete dropdown prefix-matches against, e.g.
    /// `"/cron add"`. Must equal (or be `"<bare token> <subcommand>"` of) one
    /// of the top-level `match cmd` arms in `handle_slash_command`.
    pub token: &'static str,
    /// Appended after `token` in `/help` output only — empty if the bare
    /// token needs no further explanation.
    pub usage: &'static str,
    pub description: &'static str,
}

pub const SLASH_COMMANDS: &[SlashCommandSpec] = &[
    SlashCommandSpec {
        token: "/help",
        usage: "",
        description: "Show this help",
    },
    SlashCommandSpec {
        token: "/fast",
        usage: "[on|off]",
        description: "Toggle fast mode (hide thinking traces)",
    },
    SlashCommandSpec {
        token: "/plan",
        usage: "[on|off|list|show <name>]",
        description: "Toggle plan mode, or list/show plans saved under .agents/plans/",
    },
    SlashCommandSpec {
        token: "/models",
        usage: "[name]",
        description: "List providers or switch provider",
    },
    SlashCommandSpec {
        token: "/clear",
        usage: "",
        description: "Clear conversation history",
    },
    SlashCommandSpec {
        token: "/cd",
        usage: "<path>",
        description: "Change workspace directory",
    },
    SlashCommandSpec {
        token: "/image",
        usage: "<path> [prompt]",
        description: "Attach image from file",
    },
    SlashCommandSpec {
        token: "/edit-image",
        usage: "<prompt>",
        description: "Edit attached image with prompt instruction",
    },
    SlashCommandSpec {
        token: "/paste",
        usage: "[prompt]",
        description: "Attach image from clipboard",
    },
    SlashCommandSpec {
        token: "Ctrl+V",
        usage: "",
        description: "Paste clipboard image as [Image #1]",
    },
    SlashCommandSpec {
        token: "↑ / ↓",
        usage: "",
        description: "Recall previously submitted input",
    },
    SlashCommandSpec {
        token: "/learn",
        usage: "<path>",
        description: "Import a persistent .md or .txt skill",
    },
    SlashCommandSpec {
        token: "/skill",
        usage: "[list]",
        description: "List skills (global/workspace/taught) — see also /skill add",
    },
    SlashCommandSpec {
        token: "/skill add",
        usage: "<path|github-repo|url>",
        description: "Install a local skill, or pull one via `npx skills` (GitHub repo/URL)",
    },
    SlashCommandSpec {
        token: "/plugins",
        usage: "[name] [prompt]",
        description: "List configured plugins, or generate a skill.md for one",
    },
    SlashCommandSpec {
        token: "/memory",
        usage: "",
        description: "Manage long-term memory store",
    },
    SlashCommandSpec {
        token: "/memory list",
        usage: "",
        description: "Show recent interactions",
    },
    SlashCommandSpec {
        token: "/memory clear",
        usage: "",
        description: "Clear all interactions",
    },
    SlashCommandSpec {
        token: "/memory get",
        usage: "<key>",
        description: "Read a profile value",
    },
    SlashCommandSpec {
        token: "/memory set",
        usage: "<key> <val>",
        description: "Store a profile value",
    },
    SlashCommandSpec {
        token: "/autoskill",
        usage: "[on|off]",
        description: "Toggle auto-writing a SKILL.md after hard tasks",
    },
    SlashCommandSpec {
        token: "/cron",
        usage: "",
        description: "Create/list/remove/enable/disable scheduled agent tasks",
    },
    SlashCommandSpec {
        token: "/cron add",
        usage: "<name> | <sched> | <task>",
        description: "Create a scheduled task — walks through a wizard if you don't type more",
    },
    SlashCommandSpec {
        token: "/link",
        usage: "",
        description: "Link a folder chat can auto-write notes into",
    },
    SlashCommandSpec {
        token: "/link add",
        usage: "<name> | <path> | <desc>",
        description: "Link a folder chat can auto-write notes into",
    },
    SlashCommandSpec {
        token: "/image-provider",
        usage: "[name]",
        description: "List image gen providers or switch default provider",
    },
    SlashCommandSpec {
        token: "/generate-image",
        usage: "<prompt>",
        description: "Generate image using AI model",
    },
    SlashCommandSpec {
        token: "/gen-image",
        usage: "<prompt>",
        description: "Generate image using AI model",
    },
    SlashCommandSpec {
        token: "/veo",
        usage: "<prompt>",
        description: "Generate video using Google Veo",
    },
    SlashCommandSpec {
        token: "/video-provider",
        usage: "[name]",
        description: "List video gen providers or switch default provider",
    },
    SlashCommandSpec {
        token: "/search-provider",
        usage: "[name]",
        description: "List web search providers or switch default provider",
    },
    SlashCommandSpec {
        token: "/bg",
        usage: "<query>",
        description: "Run a query in the background, non-blocking",
    },
    SlashCommandSpec {
        token: "/jobs",
        usage: "[show|cancel <id>]",
        description: "List, inspect, or cancel background jobs",
    },
    SlashCommandSpec {
        token: "/shells",
        usage: "",
        description: "List, inspect, or kill background shell jobs run_shell started",
    },
    SlashCommandSpec {
        token: "/mcp",
        usage: "",
        description: "List configured MCP servers",
    },
    SlashCommandSpec {
        token: "/mcp allow",
        usage: "<server> <tool>",
        description: "Allow an MCP tool",
    },
    SlashCommandSpec {
        token: "/mcp reauth",
        usage: "<server>",
        description: "Re-run a server's OAuth login (e.g. expired token)",
    },
    SlashCommandSpec {
        token: "/subagent",
        usage: "",
        description: "List/create/remove subagents the agent can delegate to",
    },
    SlashCommandSpec {
        token: "/subagent add",
        usage: "",
        description: "Create a subagent — walks through a wizard",
    },
    SlashCommandSpec {
        token: "/release-notes",
        usage: "",
        description: "Show release notes for the current version",
    },
    SlashCommandSpec {
        token: "/stats",
        usage: "",
        description: "Show session statistics",
    },
    SlashCommandSpec {
        token: "/exit",
        usage: "",
        description: "Exit Mint",
    },
    SlashCommandSpec {
        token: "/code",
        usage: "<task>",
        description: "Run in code-agent mode",
    },
    SlashCommandSpec {
        token: "/n8n",
        usage: "[task]",
        description: "Open n8n in your browser, or trigger a workflow via the n8n MCP server",
    },
    SlashCommandSpec {
        token: "/notebook",
        usage: "[task]",
        description: "Open SurfSense in your browser, or run a task via the surfsense MCP server",
    },
    SlashCommandSpec {
        token: "/multi-agent",
        usage: "[on|off]",
        description: "Show or toggle Multi-Agent Collaboration system",
    },
    SlashCommandSpec {
        token: "/avatar",
        usage: "[web|desktop|status|off]",
        description: "Connect agent activity to Project Avatar (real-time 3D avatar)",
    },
];

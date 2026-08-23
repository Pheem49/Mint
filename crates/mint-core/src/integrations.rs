//! External-system bridges: messaging platforms, MCP servers, OAuth
//! providers, native plugins, git hooks, and n8n/webhook workflows.

pub mod bridge_health;
pub mod channels;
pub mod hooks;
pub mod mcp;
pub mod oauth;
pub mod plugins;
pub mod workflows;

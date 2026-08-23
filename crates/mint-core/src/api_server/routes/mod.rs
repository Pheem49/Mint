pub(super) mod auth;
pub(super) mod chat;
pub(super) mod cron_mcp;
pub(super) mod gemini_live;
pub(super) mod linked_folders;
pub(super) mod media_gen;
pub(super) mod misc;
pub(super) mod profile_oauth;
pub(super) mod sessions;
pub(super) mod skills_subagents;
pub(super) mod status_health;

/// Parsed pieces of a request, shared across every route handler — grouped
/// into one struct rather than passed as loose parameters so adding a route
/// handler doesn't also mean touching every call site's argument list (and
/// so `execute` itself doesn't trip clippy's `too_many_arguments`).
pub(super) struct RequestCtx<'a> {
    pub(super) method: &'a str,
    pub(super) route: &'a str,
    pub(super) query: &'a str,
    pub(super) body: &'a str,
    pub(super) request_str: &'a str,
    pub(super) request_bytes: &'a [u8],
    pub(super) header_end: usize,
    pub(super) auth_label: String,
}

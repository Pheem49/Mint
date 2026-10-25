//! What a sub-agent may do. Derived from its base profile's `tools:` list and
//! nothing else — a model-supplied role name never widens it.
use std::collections::BTreeSet;

/// Actions known to be pure reads. Anything *not* listed here is treated as
/// able to mutate state — an allow-list, so a newly added tool defaults to the
/// safe (serialized) side instead of silently running in parallel.
pub const READ_ONLY_ACTIONS: &[&str] = &[
    "list_files",
    "read_file",
    "search_code",
    "symbols",
    "find_definition",
    "find_references",
    "repo_map",
    "semantic_search",
    "knowledge_search",
    "web_search",
    "memory_recall",
    "git_status",
    "git_diff",
    "git_log",
    "git_branch",
    "detect_project",
    "list_tests",
    "read_diagnostics",
    "view_image",
    "mcp_list_tools",
    "conversation_summary",
    "finish",
];

/// Tools a sub-agent can never use, whatever its profile says: spawning more
/// agents (depth limit — subagents never dispatch subagents).
const NEVER_ALLOWED: &[&str] = &["dispatch_agents", "dispatch_subagent"];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentPermissions {
    /// `None` = the profile declared no restriction (full catalog).
    allowed: Option<BTreeSet<String>>,
}

impl AgentPermissions {
    pub fn unrestricted() -> Self {
        Self { allowed: None }
    }

    pub fn from_tools(tools: Option<&[String]>) -> Self {
        let allowed = tools.map(|list| {
            let mut set: BTreeSet<String> = list
                .iter()
                .map(|t| t.trim().to_string())
                .filter(|t| !t.is_empty() && !NEVER_ALLOWED.contains(&t.as_str()))
                .collect();
            // An agent must always be able to hand its result back.
            set.insert("finish".to_string());
            set
        });
        Self { allowed }
    }

    pub fn allows(&self, action: &str) -> bool {
        if NEVER_ALLOWED.contains(&action) {
            return false;
        }
        match &self.allowed {
            None => true,
            Some(set) => set.contains(action),
        }
    }

    /// Whether this agent could change workspace/system state and must
    /// therefore take the single write lane.
    pub fn is_write_capable(&self) -> bool {
        match &self.allowed {
            None => true,
            Some(set) => set.iter().any(|a| !READ_ONLY_ACTIONS.contains(&a.as_str())),
        }
    }

    pub fn allowed_actions(&self) -> Option<&BTreeSet<String>> {
        self.allowed.as_ref()
    }
}

tokio::task_local! {
    static CURRENT_PERMISSIONS: Option<AgentPermissions>;
}

pub async fn scope_permissions<F: std::future::Future>(
    permissions: Option<AgentPermissions>,
    future: F,
) -> F::Output {
    CURRENT_PERMISSIONS.scope(permissions, future).await
}

pub fn current_permissions() -> Option<AgentPermissions> {
    CURRENT_PERMISSIONS.try_with(|p| p.clone()).ok().flatten()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::subagents::builtin_subagents;

    fn builtin_permissions(name: &str) -> AgentPermissions {
        let def = builtin_subagents()
            .into_iter()
            .find(|d| d.name == name)
            .unwrap_or_else(|| panic!("no builtin {name}"));
        AgentPermissions::from_tools(def.tools.as_deref())
    }

    #[test]
    fn explorer_is_read_only_and_cannot_write_or_shell() {
        let p = builtin_permissions("explorer");
        assert!(p.allows("read_file"));
        assert!(p.allows("search_code"));
        assert!(!p.allows("write_file"));
        assert!(!p.allows("apply_patch"));
        assert!(!p.allows("run_shell"));
        assert!(!p.is_write_capable());
    }

    #[test]
    fn debugger_may_run_commands_but_not_edit_files() {
        let p = builtin_permissions("debugger");
        assert!(p.allows("run_shell"));
        assert!(!p.allows("write_file"));
        assert!(!p.allows("apply_patch"));
        assert!(p.is_write_capable(), "run_shell can mutate, so it takes the write lane");
    }

    #[test]
    fn unrestricted_profiles_are_write_capable() {
        let p = AgentPermissions::unrestricted();
        assert!(p.allows("write_file"));
        assert!(p.is_write_capable());
    }

    #[test]
    fn finish_is_always_allowed_even_with_an_empty_list() {
        let p = AgentPermissions::from_tools(Some(&[]));
        assert!(p.allows("finish"));
        assert!(!p.allows("read_file"));
        assert!(!p.is_write_capable());
    }

    #[test]
    fn spawning_agents_is_never_allowed_even_when_unrestricted() {
        for p in [
            AgentPermissions::unrestricted(),
            AgentPermissions::from_tools(Some(&["dispatch_agents".to_string()])),
        ] {
            assert!(!p.allows("dispatch_agents"));
            assert!(!p.allows("dispatch_subagent"));
        }
    }

    #[test]
    fn an_unknown_tool_in_the_list_counts_as_write_capable() {
        let p = AgentPermissions::from_tools(Some(&["brand_new_tool".to_string()]));
        assert!(p.is_write_capable());
    }
}

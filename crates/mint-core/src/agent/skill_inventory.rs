//! Learned skills from database, global files and the selected workspace.

use crate::{LearnedSkill, MemoryStore, skills::SkillError};
use serde::Serialize;
use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum SkillLocation {
    Database,
    Global,
    Workspace,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AvailableSkill {
    #[serde(flatten)]
    pub skill: LearnedSkill,
    pub location: SkillLocation,
    pub updated_at: String,
}

impl From<LearnedSkill> for AvailableSkill {
    fn from(skill: LearnedSkill) -> Self {
        let updated_at = skill.created_at.clone();
        Self {
            skill,
            location: SkillLocation::Database,
            updated_at,
        }
    }
}

pub fn list_available_skills(
    memory: &MemoryStore,
    workspace: Option<&Path>,
) -> Result<Vec<AvailableSkill>, SkillError> {
    inventory_from(memory, dirs::home_dir().as_deref(), workspace)
}

/// Mandatory rules compose by source path; picker name precedence only selects optional skills.
pub(super) fn skills_for_context(
    memory: &MemoryStore,
    workspace: Option<&Path>,
) -> Result<Vec<LearnedSkill>, SkillError> {
    context_from(memory, dirs::home_dir().as_deref(), workspace)
}

fn context_from(
    memory: &MemoryStore,
    home: Option<&Path>,
    workspace: Option<&Path>,
) -> Result<Vec<LearnedSkill>, SkillError> {
    let mut rules = std::collections::BTreeMap::new();
    let mut skills = std::collections::BTreeMap::new();
    for entry in discover_from(memory, home, workspace)? {
        let skill = entry.skill;
        let path = Path::new(&skill.source_path);
        if crate::skills::is_rules_file(path) {
            let canonical = path.canonicalize().unwrap_or_else(|_| path.to_owned());
            rules.insert(canonical, skill);
        } else {
            skills.insert(skill.name.clone(), skill);
        }
    }
    Ok(rules.into_values().chain(skills.into_values()).collect())
}

fn inventory_from(
    memory: &MemoryStore,
    home: Option<&Path>,
    workspace: Option<&Path>,
) -> Result<Vec<AvailableSkill>, SkillError> {
    let mut selected = std::collections::BTreeMap::new();
    for entry in discover_from(memory, home, workspace)? {
        selected.insert(entry.skill.name.clone(), entry);
    }
    Ok(selected.into_values().collect())
}

fn discover_from(
    memory: &MemoryStore,
    home: Option<&Path>,
    workspace: Option<&Path>,
) -> Result<Vec<AvailableSkill>, SkillError> {
    use crate::skills::{load_agent_rules_file, load_skills_from_dir};
    let mut discovered = Vec::new();
    let mut add_layer = |mut skills: Vec<LearnedSkill>, location| {
        // File discovery order must not choose an arbitrary duplicate winner.
        skills.sort_by(|a, b| a.source_path.cmp(&b.source_path));
        for mut skill in skills {
            skill.is_workspace = location == SkillLocation::Workspace;
            let mut entry = AvailableSkill::from(skill);
            entry.location = location;
            discovered.push(entry);
        }
    };
    add_layer(memory.learned_skills(100)?, SkillLocation::Database);
    if let Some(home) = home {
        let mut rules = Vec::new();
        load_agent_rules_file(&home.join(".gemini/config/AGENTS.md"), &mut rules);
        add_layer(rules, SkillLocation::Global);
        let mut skills = Vec::new();
        load_skills_from_dir(&home.join(".config/mint/mint-skills"), &mut skills);
        add_layer(skills, SkillLocation::Global);
    }
    if let Some(root) = workspace
        .and_then(|path| path.canonicalize().ok())
        .filter(|path| path.is_dir())
    {
        let root = if root.ends_with("src-tauri") {
            root.parent().unwrap_or(&root)
        } else {
            &root
        };
        for path in [
            ".agents/AGENTS.md",
            "AGENTS.md",
            ".claude/CLAUDE.md",
            "CLAUDE.md",
            ".cursorrules",
            ".github/copilot-instructions.md",
        ] {
            let mut rules = Vec::new();
            load_agent_rules_file(&root.join(path), &mut rules);
            add_layer(rules, SkillLocation::Workspace);
        }
        let mut rules = Vec::new();
        if let Ok(entries) = std::fs::read_dir(root.join(".cursor/rules")) {
            for path in entries.flatten().map(|entry| entry.path()).filter(|path| {
                matches!(
                    path.extension().and_then(|e| e.to_str()),
                    Some("md" | "mdc")
                )
            }) {
                load_agent_rules_file(&path, &mut rules);
            }
        }
        add_layer(rules, SkillLocation::Workspace);
        for path in [".agents/skills", "skills"] {
            let mut skills = Vec::new();
            load_skills_from_dir(&root.join(path), &mut skills);
            add_layer(skills, SkillLocation::Workspace);
        }
    }
    Ok(discovered)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{fs, path::PathBuf};

    struct Fixture {
        root: PathBuf,
        memory: MemoryStore,
    }
    impl Fixture {
        fn new() -> Self {
            let root =
                std::env::temp_dir().join(format!("mint-inventory-{}", uuid::Uuid::new_v4()));
            fs::create_dir_all(&root).unwrap();
            let memory = MemoryStore::open(root.join("memory.sqlite"));
            Self { root, memory }
        }
        fn file(&self, path: &str, content: &str) {
            let path = self.root.join(path);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(path, content).unwrap();
        }
        fn list(&self) -> Vec<AvailableSkill> {
            inventory_from(
                &self.memory,
                Some(&self.root.join("home")),
                Some(&self.root.join("workspace")),
            )
            .unwrap()
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.root);
        }
    }

    #[test]
    fn review_context_keeps_all_mandatory_rule_sources_while_skills_use_precedence() {
        let f = Fixture::new();
        for (path, content) in [
            ("home/.gemini/config/AGENTS.md", "global required rule"),
            ("workspace/.agents/AGENTS.md", "nested required rule"),
            ("workspace/AGENTS.md", "root required rule"),
            ("workspace/.claude/CLAUDE.md", "nested claude rule"),
            ("workspace/CLAUDE.md", "root claude rule"),
            (
                "home/.config/mint/mint-skills/common/SKILL.md",
                "global skill",
            ),
            ("workspace/skills/common.md", "workspace skill"),
        ] {
            f.file(path, content);
        }
        let skills = context_from(
            &f.memory,
            Some(&f.root.join("home")),
            Some(&f.root.join("workspace")),
        )
        .unwrap();
        assert_eq!(skills.len(), 6);
        for content in [
            "global required rule",
            "nested required rule",
            "root required rule",
            "nested claude rule",
            "root claude rule",
            "workspace skill",
        ] {
            assert!(
                skills.iter().any(|skill| skill.content == content),
                "dropped {content}"
            );
        }
        assert!(!skills.iter().any(|skill| skill.content == "global skill"));
    }

    #[test]
    fn core_migration_workspace_wins_over_global_and_database() {
        let f = Fixture::new();
        f.memory
            .add_learned_skill("same", "manual", "database")
            .unwrap();
        f.file("home/.config/mint/mint-skills/same/SKILL.md", "global");
        f.file("workspace/.agents/skills/same/SKILL.md", "workspace");
        let skills = f.list();
        assert_eq!(skills.len(), 1);
        assert_eq!(skills[0].skill.content, "workspace");
        assert_eq!(skills[0].location, SkillLocation::Workspace);
        assert!(skills[0].skill.is_workspace);
    }

    #[test]
    fn core_migration_global_wins_over_database_without_a_workspace() {
        let f = Fixture::new();
        f.memory
            .add_learned_skill("same", "manual", "database")
            .unwrap();
        f.file("home/.config/mint/mint-skills/same/SKILL.md", "global");
        let skills = inventory_from(&f.memory, Some(&f.root.join("home")), None).unwrap();
        assert_eq!(skills.len(), 1);
        assert_eq!(skills[0].skill.content, "global");
        assert_eq!(skills[0].location, SkillLocation::Global);
        assert!(!skills[0].skill.is_workspace);
    }

    #[test]
    fn core_migration_selected_workspace_does_not_leak_another_workspace() {
        let f = Fixture::new();
        f.file("workspace/skills/first.md", "first workspace");
        f.file("other/skills/second.md", "second workspace");
        let skills = inventory_from(&f.memory, None, Some(&f.root.join("other"))).unwrap();
        assert_eq!(
            skills
                .iter()
                .map(|s| s.skill.name.as_str())
                .collect::<Vec<_>>(),
            ["second"]
        );
    }

    #[test]
    fn core_migration_root_rules_and_skills_override_dot_agents_versions() {
        let f = Fixture::new();
        f.file("workspace/.agents/AGENTS.md", "nested rules");
        f.file("workspace/AGENTS.md", "root rules");
        f.file("workspace/.agents/skills/common/SKILL.md", "nested skill");
        f.file("workspace/skills/common.md", "root skill");
        let skills = f.list();
        assert_eq!(
            skills
                .iter()
                .map(|s| (s.skill.name.as_str(), s.skill.content.as_str()))
                .collect::<Vec<_>>(),
            [("AGENTS.md", "root rules"), ("common", "root skill")]
        );
    }

    #[test]
    fn core_migration_inventory_retains_rule_sources_used_by_agent_context() {
        let f = Fixture::new();
        f.file("workspace/CLAUDE.md", "claude rules");
        f.file("workspace/.cursor/rules/custom.mdc", "cursor rules");
        f.file("workspace/.github/copilot-instructions.md", "copilot rules");
        let skills = f.list();
        assert_eq!(
            skills
                .iter()
                .map(|s| s.skill.name.as_str())
                .collect::<Vec<_>>(),
            ["CLAUDE.md", "copilot-instructions.md", "custom.mdc"]
        );
    }

    #[test]
    fn core_migration_wire_metadata_preserves_database_timestamp_and_location() {
        let f = Fixture::new();
        let saved = f
            .memory
            .add_learned_skill(
                "manual",
                "ui_manual",
                "---\ndescription: Useful skill\n---\nbody",
            )
            .unwrap();
        let skills = f.list();
        assert_eq!(skills.len(), 1);
        let wire = serde_json::to_value(&skills[0]).unwrap();
        assert_eq!(wire["updatedAt"], saved.created_at);
        assert_eq!(wire["createdAt"], saved.created_at);
        assert_eq!(wire["location"], "database");
        assert_eq!(wire["sourcePath"], "ui_manual");
        assert_eq!(wire["description"], "Useful skill");
        assert_eq!(wire["is_workspace"], false);
    }
}

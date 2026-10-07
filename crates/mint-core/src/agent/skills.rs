use std::{fs, path::Path};

use thiserror::Error;

use crate::{LearnedSkill, MemoryError, MemoryStore};

const MAX_SKILL_BYTES: u64 = 256 * 1024;
const MAX_CONTEXT_BYTES: usize = 64 * 1024;

#[derive(Debug, Error)]
pub enum SkillError {
    #[error("unable to resolve skill file {path}: {source}")]
    Resolve {
        path: String,
        source: std::io::Error,
    },
    #[error("skill path is not a file: {0}")]
    NotFile(String),
    #[error("skill file is too large ({0} bytes); limit is {MAX_SKILL_BYTES} bytes")]
    TooLarge(u64),
    #[error("Mint learn supports .md and .txt files only")]
    UnsupportedExtension,
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Memory(#[from] MemoryError),
}

pub fn learn_skill(path: &Path) -> Result<LearnedSkill, SkillError> {
    let path = path.canonicalize().map_err(|source| SkillError::Resolve {
        path: path.display().to_string(),
        source,
    })?;
    let metadata = fs::metadata(&path)?;
    if !metadata.is_file() {
        return Err(SkillError::NotFile(path.display().to_string()));
    }
    if metadata.len() > MAX_SKILL_BYTES {
        return Err(SkillError::TooLarge(metadata.len()));
    }
    if !path
        .extension()
        .and_then(|value| value.to_str())
        .is_some_and(|extension| matches!(extension.to_ascii_lowercase().as_str(), "md" | "txt"))
    {
        return Err(SkillError::UnsupportedExtension);
    }
    let content = fs::read_to_string(&path)?;
    let name = path
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("skill");
    Ok(MemoryStore::open_default()?.add_learned_skill(name, &path.to_string_lossy(), &content)?)
}

pub fn load_agent_rules_file(file_path: &Path, list: &mut Vec<LearnedSkill>) {
    if file_path.is_file() {
        if let Ok(content) = fs::read_to_string(file_path) {
            let name = file_path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("AGENTS.md")
                .to_string();
            let description = parse_skill_description(&content)
                .or_else(|| Some("Workspace instructions and rules".to_string()));
            list.push(LearnedSkill {
                id: 0,
                name,
                source_path: file_path.to_string_lossy().to_string(),
                content,
                created_at: String::new(),
                description,
                is_workspace: false,
            });
        }
    }
}

pub fn learned_skills_context(
    workspace_root: Option<&Path>,
    chat_id: Option<&str>,
) -> Result<String, SkillError> {
    learned_skills_context_with_history(workspace_root, chat_id, "")
}

pub fn learned_skills_context_with_history(
    workspace_root: Option<&Path>,
    chat_id: Option<&str>,
    history: &str,
) -> Result<String, SkillError> {
    let mut skills = MemoryStore::open_default()?.learned_skills(100)?;

    if let Some(home) = dirs::home_dir() {
        let global_agents_path = home.join(".gemini").join("config").join("AGENTS.md");
        load_agent_rules_file(&global_agents_path, &mut skills);

        let global_skills_path = home.join(".config").join("mint").join("mint-skills");
        if !global_skills_path.exists() {
            let _ = std::fs::create_dir_all(&global_skills_path);
        }
        load_skills_from_dir(&global_skills_path, &mut skills);
    }

    if let Some(root) = workspace_root {
        let rule_files = [
            root.join(".agents").join("AGENTS.md"),
            root.join("AGENTS.md"),
            root.join(".claude").join("CLAUDE.md"),
            root.join("CLAUDE.md"),
            root.join(".cursorrules"),
            root.join(".github").join("copilot-instructions.md"),
        ];
        for rule_path in &rule_files {
            if rule_path.is_file() {
                load_agent_rules_file(rule_path, &mut skills);
            }
        }

        let cursor_rules_dir = root.join(".cursor").join("rules");
        if let Ok(entries) = std::fs::read_dir(cursor_rules_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_file()
                    && (path.extension().and_then(|e| e.to_str()) == Some("mdc")
                        || path.extension().and_then(|e| e.to_str()) == Some("md"))
                {
                    load_agent_rules_file(&path, &mut skills);
                }
            }
        }

        let workspace_skills_path1 = root.join(".agents").join("skills");
        load_skills_from_dir(&workspace_skills_path1, &mut skills);

        let workspace_skills_path2 = root.join("skills");
        load_skills_from_dir(&workspace_skills_path2, &mut skills);
    }

    skills_context_from_with_history(
        &MemoryStore::open_default()?,
        skills,
        workspace_root,
        chat_id,
        history,
    )
}

#[cfg(test)]
fn skills_context_from(
    memory: &MemoryStore,
    skills: Vec<LearnedSkill>,
    _workspace_root: Option<&Path>,
    chat_id: Option<&str>,
) -> Result<String, SkillError> {
    skills_context_from_with_history(memory, skills, _workspace_root, chat_id, "")
}

fn skills_context_from_with_history(
    memory: &MemoryStore,
    skills: Vec<LearnedSkill>,
    _workspace_root: Option<&Path>,
    chat_id: Option<&str>,
    history: &str,
) -> Result<String, SkillError> {
    let mut unique_skills = std::collections::BTreeMap::new();
    for mut skill in skills {
        let path = Path::new(&skill.source_path);
        if let Ok(canonical) = path.canonicalize() {
            skill.source_path = canonical.to_string_lossy().into_owned();
        }
        unique_skills.insert(skill.source_path.clone(), skill);
    }
    let mut catalog = String::from(
        "Choose only skills relevant to the task by their name and description. Before using an UNREAD skill, execute read_file on its Path without line limits. Do not read unrelated skills. READ means the exact unchanged content is retained in this request.\n\n",
    );
    let mut retained = String::new();
    for skill in unique_skills.into_values() {
        // Rules apply to every task; they are not optional skills.
        if is_rules_file(Path::new(&skill.source_path)) {
            if let Ok(content) = fs::read_to_string(&skill.source_path) {
                retained.push_str(&format!(
                    "Workspace instructions: {}\n{}\n\n",
                    skill.source_path, content
                ));
            }
            continue;
        }
        let current = fs::read_to_string(&skill.source_path).ok();
        let saved = match chat_id {
            Some(id) => memory.skill_read_content(id, &skill.source_path)?,
            None => None,
        };
        let available = current
            .as_ref()
            .filter(|content| saved.as_ref() == Some(*content));
        let body = available.map(|content| {
            format!(
                "Loaded skill: {}\nPath: {}\n{}\n\n",
                skill.name, skill.source_path, content
            )
        });
        // Never mark a skill READ if its full contents won't be sent to the model.
        let visible = available.is_some_and(|content| {
            history.contains(&format!("[Loaded skill: {}]\n{}", skill.name, content))
        });
        let replayable = body
            .as_ref()
            .is_some_and(|body| retained.len() + body.len() <= MAX_CONTEXT_BYTES);
        let loaded = visible || replayable;
        if replayable && !visible {
            retained.push_str(body.as_ref().unwrap());
        }
        let description = current
            .as_deref()
            .and_then(parse_skill_description)
            .or(skill.description)
            .unwrap_or_else(|| "No description provided".into());
        catalog.push_str(&format!(
            "Skill: {}\nDescription: {}\nPath: {}\nStatus: {}\n\n",
            skill.name,
            description,
            skill.source_path,
            if loaded {
                "READ — full unchanged content retained in this request."
            } else {
                "UNREAD — read this file through read_file before using it."
            }
        ));
    }
    Ok(format!("{catalog}\n{retained}"))
}

fn is_rules_file(path: &Path) -> bool {
    matches!(
        path.file_name().and_then(|name| name.to_str()),
        Some("AGENTS.md" | "CLAUDE.md" | ".cursorrules" | "copilot-instructions.md")
    ) || path
        .parent()
        .is_some_and(|parent| parent.ends_with(".cursor/rules"))
}

/// Resolve only a file the skill catalog actually advertises, including taught/global skills.
pub fn skill_for_read_path(root: &Path, path: &str) -> Option<LearnedSkill> {
    let canonical = root.join(path).canonicalize().ok()?;
    if is_rules_file(&canonical) {
        return None;
    }
    let mut skills = MemoryStore::open_default().ok()?.learned_skills(100).ok()?;
    if let Some(home) = dirs::home_dir() {
        load_skills_from_dir(&home.join(".config/mint/mint-skills"), &mut skills);
    }
    load_skills_from_dir(&root.join(".agents/skills"), &mut skills);
    load_skills_from_dir(&root.join("skills"), &mut skills);
    skills
        .into_iter()
        .find(|skill| {
            Path::new(&skill.source_path).canonicalize().ok().as_ref() == Some(&canonical)
        })
        .map(|mut skill| {
            skill.source_path = canonical.to_string_lossy().into_owned();
            skill
        })
}

/// Full skill reads intentionally don't use the normal 240-line source preview.
/// The receipt is saved only after a real, capability-checked read succeeds.
pub fn read_skill_file(
    memory: &MemoryStore,
    skill: &LearnedSkill,
    chat_id: &str,
    config: &crate::MintConfig,
) -> Result<String, SkillError> {
    let path = crate::assert_path_capability(
        Path::new(&skill.source_path),
        crate::Capability::Read,
        config,
    )
    .map_err(|error| std::io::Error::other(error.to_string()))?;
    let metadata = fs::metadata(&path)?;
    if metadata.len() > MAX_SKILL_BYTES {
        return Err(SkillError::TooLarge(metadata.len()));
    }
    let content = fs::read_to_string(&path)?;
    if content.len() as u64 > MAX_SKILL_BYTES {
        return Err(SkillError::TooLarge(content.len() as u64));
    }
    memory.record_skill_read(chat_id, &path.to_string_lossy(), &content)?;
    Ok(format!("[Loaded skill: {}]\n{}", skill.name, content))
}

pub fn parse_skill_description(content: &str) -> Option<String> {
    let content = content.trim_start();
    if !content.starts_with("---") {
        return None;
    }
    let rest = &content[3..];
    let end_idx = rest.find("---")?;
    let frontmatter = &rest[..end_idx];
    for line in frontmatter.lines() {
        let line = line.trim();
        if let Some(desc) = line.strip_prefix("description:") {
            let mut val = desc.trim();
            if (val.starts_with('"') && val.ends_with('"'))
                || (val.starts_with('\'') && val.ends_with('\''))
            {
                if val.len() >= 2 {
                    val = &val[1..val.len() - 1];
                }
            }
            return Some(val.to_string());
        }
    }
    None
}

pub fn load_skills_from_dir(dir: &Path, list: &mut Vec<LearnedSkill>) {
    if !dir.is_dir() {
        return;
    }
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                for filename in &["SKILL.md", "SKILL.txt", "skill.md", "skill.txt"] {
                    let skill_file = path.join(filename);
                    if skill_file.is_file() {
                        if let Ok(content) = fs::read_to_string(&skill_file) {
                            let name = path
                                .file_name()
                                .and_then(|n| n.to_str())
                                .unwrap_or("skill")
                                .to_string();
                            let description = parse_skill_description(&content);
                            list.push(LearnedSkill {
                                id: 0,
                                name,
                                source_path: skill_file.to_string_lossy().to_string(),
                                content,
                                created_at: String::new(),
                                description,
                                is_workspace: false,
                            });
                        }
                        break;
                    }
                }
            } else if path.is_file()
                && let Some(ext) = path.extension().and_then(|e| e.to_str())
                && matches!(ext.to_ascii_lowercase().as_str(), "md" | "txt")
                && let Ok(content) = fs::read_to_string(&path)
            {
                let name = path
                    .file_stem()
                    .and_then(|n| n.to_str())
                    .unwrap_or("skill")
                    .to_string();
                let description = parse_skill_description(&content);
                list.push(LearnedSkill {
                    id: 0,
                    name,
                    source_path: path.to_string_lossy().to_string(),
                    content,
                    created_at: String::new(),
                    description,
                    is_workspace: false,
                });
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture() -> (std::path::PathBuf, MemoryStore, Vec<LearnedSkill>) {
        let root = std::env::temp_dir().join(format!(
            "mint-skill-context-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let directory = root.join(".agents/skills/review");
        fs::create_dir_all(&directory).unwrap();
        fs::write(
            directory.join("SKILL.md"),
            "---\ndescription: Review code\n---\nReview every changed function.",
        )
        .unwrap();
        let mut skills = Vec::new();
        load_skills_from_dir(&root.join(".agents/skills"), &mut skills);
        let memory = MemoryStore::open(root.join("memory.sqlite"));
        (root, memory, skills)
    }

    #[test]
    fn conversation_history_does_not_mark_unread_skills_as_loaded() {
        let (root, memory, skills) = fixture();
        memory
            .add_interaction_for_chat("session-a", "hello", "hello", "", "")
            .unwrap();
        let context = skills_context_from(&memory, skills, Some(&root), Some("session-a")).unwrap();
        assert!(context.contains("Status: UNREAD"), "{context}");
        assert!(!context.contains("Review every changed function."));
    }

    #[test]
    fn global_skills_are_catalog_entries_not_preloaded_bodies() {
        let (root, memory, skills) = fixture();
        let context = skills_context_from(&memory, skills, None, Some("session-a")).unwrap();
        assert!(context.contains("Description: Review code"), "{context}");
        assert!(context.contains("Path:"));
        assert!(!context.contains("Review every changed function."));
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn successful_reads_are_scoped_to_session_and_changed_files_need_new_reads() {
        let (root, memory, skills) = fixture();
        let skill = &skills[0];
        memory
            .record_skill_read("session-a", &skill.source_path, &skill.content)
            .unwrap();
        let loaded =
            skills_context_from(&memory, skills.clone(), Some(&root), Some("session-a")).unwrap();
        assert!(loaded.contains("Status: READ"), "{loaded}");
        assert!(loaded.contains("Review every changed function."));
        let other =
            skills_context_from(&memory, skills.clone(), Some(&root), Some("session-b")).unwrap();
        assert!(other.contains("Status: UNREAD"));
        assert!(!other.contains("Review every changed function."));
        fs::write(
            &skill.source_path,
            "---\ndescription: Review code\n---\nNew instructions.",
        )
        .unwrap();
        let changed =
            skills_context_from(&memory, skills.clone(), Some(&root), Some("session-a")).unwrap();
        assert!(changed.contains("Status: UNREAD"), "{changed}");
        assert!(!changed.contains("Review every changed function."));
        memory.clear_interactions_for_chat("session-a").unwrap();
        assert!(
            memory
                .skill_read_content("session-a", &skill.source_path)
                .unwrap()
                .is_none()
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn full_read_returns_all_lines_and_records_only_successful_reads() {
        let (root, memory, mut skills) = fixture();
        let content = (1..=300)
            .map(|i| format!("Instruction {i}\n"))
            .collect::<String>();
        fs::write(&skills[0].source_path, &content).unwrap();
        let config = crate::MintConfig {
            allowed_read_paths: vec![root.clone()],
            blocked_paths: vec![],
            ..Default::default()
        };
        let result = read_skill_file(&memory, &skills[0], "session-a", &config).unwrap();
        assert!(result.contains("Instruction 300\n"));
        assert_eq!(
            memory
                .skill_read_content("session-a", &skills[0].source_path)
                .unwrap(),
            Some(content)
        );
        let denied = crate::MintConfig {
            allowed_read_paths: vec![],
            ..config.clone()
        };
        assert!(read_skill_file(&memory, &skills[0], "denied", &denied).is_err());
        assert!(
            memory
                .skill_read_content("denied", &skills[0].source_path)
                .unwrap()
                .is_none()
        );
        // Missing paths do not become receipts either.
        skills[0].source_path = root.join("missing.md").to_string_lossy().into_owned();
        assert!(read_skill_file(&memory, &skills[0], "missing", &config).is_err());
        assert!(
            memory
                .skill_read_content("missing", &skills[0].source_path)
                .unwrap()
                .is_none()
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn receipts_not_included_in_context_are_not_marked_read() {
        let (root, memory, skills) = fixture();
        let large = "x".repeat(MAX_CONTEXT_BYTES + 1);
        fs::write(&skills[0].source_path, &large).unwrap();
        memory
            .record_skill_read("a", &skills[0].source_path, &large)
            .unwrap();
        let context = skills_context_from(&memory, skills, Some(&root), Some("a")).unwrap();
        assert!(context.contains("Status: UNREAD"));
        assert!(!context.contains(&large));
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn project_rules_are_present_without_being_optional_skills() {
        let (root, memory, mut skills) = fixture();
        fs::write(root.join("AGENTS.md"), "Always preserve user changes.").unwrap();
        load_agent_rules_file(&root.join("AGENTS.md"), &mut skills);
        let context = skills_context_from(&memory, skills, Some(&root), Some("a")).unwrap();
        assert!(context.contains("Always preserve user changes."));
        assert!(!context.contains("Skill: AGENTS.md"));
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn reading_one_skill_does_not_mark_another_read_and_receipts_survive_reopen() {
        let (root, memory, mut skills) = fixture();
        let other = root.join("skills/build/SKILL.md");
        fs::create_dir_all(other.parent().unwrap()).unwrap();
        fs::write(
            &other,
            "---\ndescription: Build projects\n---\nRun the build.",
        )
        .unwrap();
        load_skills_from_dir(&root.join("skills"), &mut skills);
        let config = crate::MintConfig {
            allowed_read_paths: vec![root.clone()],
            blocked_paths: vec![],
            ..Default::default()
        };
        read_skill_file(&memory, &skills[0], "a", &config).unwrap();
        let reopened = MemoryStore::open(root.join("memory.sqlite"));
        let context = skills_context_from(&reopened, skills, Some(&root), Some("a")).unwrap();
        assert!(context.contains("Review every changed function."));
        assert!(!context.contains("Run the build."));
        let build = context.split("Skill: build\n").nth(1).unwrap();
        assert!(
            build
                .split("\n\n")
                .next()
                .unwrap()
                .contains("Status: UNREAD")
        );
        memory.delete_chat_session("a").unwrap();
        assert!(
            reopened
                .skill_read_content(
                    "a",
                    &root
                        .join(".agents/skills/review/SKILL.md")
                        .to_string_lossy()
                )
                .unwrap()
                .is_none()
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn test_fs_skills_loading() {
        let test_dir = std::env::temp_dir().join("mint_skills_test_unique_dir");
        let _ = std::fs::remove_dir_all(&test_dir);
        let workspace_path = test_dir.join(".agents").join("skills");
        std::fs::create_dir_all(&workspace_path).unwrap();

        // Write a file skill
        let file_skill_path = workspace_path.join("rust-style.md");
        std::fs::write(&file_skill_path, "Use 4 spaces for Rust indent.").unwrap();

        // Write a directory skill
        let dir_skill_path = workspace_path.join("js-style");
        std::fs::create_dir_all(&dir_skill_path).unwrap();
        std::fs::write(dir_skill_path.join("SKILL.md"), "Use 2 spaces for JS.").unwrap();

        let mut list = Vec::new();
        load_skills_from_dir(&workspace_path, &mut list);

        let _ = std::fs::remove_dir_all(&test_dir);

        assert_eq!(list.len(), 2);

        let rust_skill = list.iter().find(|s| s.name == "rust-style").unwrap();
        assert_eq!(rust_skill.content, "Use 4 spaces for Rust indent.");

        let js_skill = list.iter().find(|s| s.name == "js-style").unwrap();
        assert_eq!(js_skill.content, "Use 2 spaces for JS.");
    }
    #[test]
    fn large_skill_already_visible_in_request_is_read_without_cached_replay() {
        let (root, memory, skills) = fixture();
        let content = "instruction\n".repeat(8000);
        fs::write(&skills[0].source_path, &content).unwrap();
        memory
            .record_skill_read("a", &skills[0].source_path, &content)
            .unwrap();
        let history = format!("[Loaded skill: {}]\n{}", skills[0].name, content);
        let context = skills_context_from_with_history(
            &memory,
            skills.clone(),
            Some(&root),
            Some("a"),
            &history,
        )
        .unwrap();
        assert!(context.contains("Status: READ"));
        let lost = skills_context_from_with_history(
            &memory,
            skills,
            Some(&root),
            Some("a"),
            "summary only",
        )
        .unwrap();
        assert!(lost.contains("Status: UNREAD"));
    }
}

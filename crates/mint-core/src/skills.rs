use std::{fs, path::Path};

use thiserror::Error;

use crate::{LearnedSkill, MemoryError, MemoryStore};

const MAX_SKILL_BYTES: u64 = 256 * 1024;
const MAX_CONTEXT_BYTES: usize = 16 * 1024;

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

pub fn learned_skills_context(workspace_root: Option<&Path>) -> Result<String, SkillError> {
    let mut skills = MemoryStore::open_default()?.learned_skills(20)?;

    if let Some(home) = dirs::home_dir() {
        let global_skills_path = home.join(".config").join("mint").join("mint-skills");
        if !global_skills_path.exists() {
            let _ = std::fs::create_dir_all(&global_skills_path);
        }
        load_skills_from_dir(&global_skills_path, &mut skills);
    }

    if let Some(root) = workspace_root {
        let workspace_skills_path1 = root.join(".agents").join("skills");
        load_skills_from_dir(&workspace_skills_path1, &mut skills);

        let workspace_skills_path2 = root.join("skills");
        load_skills_from_dir(&workspace_skills_path2, &mut skills);
    }

    let mut unique_skills = std::collections::BTreeMap::new();
    for skill in skills {
        unique_skills.insert(skill.name.clone(), skill);
    }

    let value = unique_skills
        .into_values()
        .map(|skill| format!("Skill: {}\n{}", skill.name, skill.content))
        .collect::<Vec<_>>()
        .join("\n\n");
    if value.len() <= MAX_CONTEXT_BYTES {
        return Ok(value);
    }
    let mut end = MAX_CONTEXT_BYTES;
    while !value.is_char_boundary(end) {
        end -= 1;
    }
    Ok(format!("{}\n...<learned skills truncated>", &value[..end]))
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
                            list.push(LearnedSkill {
                                id: 0,
                                name,
                                source_path: skill_file.to_string_lossy().to_string(),
                                content,
                                created_at: String::new(),
                            });
                        }
                        break;
                    }
                }
            } else if path.is_file() {
                if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
                    if matches!(ext.to_ascii_lowercase().as_str(), "md" | "txt") {
                        if let Ok(content) = fs::read_to_string(&path) {
                            let name = path
                                .file_stem()
                                .and_then(|n| n.to_str())
                                .unwrap_or("skill")
                                .to_string();
                            list.push(LearnedSkill {
                                id: 0,
                                name,
                                source_path: path.to_string_lossy().to_string(),
                                content,
                                created_at: String::new(),
                            });
                        }
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
}

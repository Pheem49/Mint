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

pub fn learned_skills_context() -> Result<String, SkillError> {
    let skills = MemoryStore::open_default()?.learned_skills(20)?;
    let value = skills
        .into_iter()
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

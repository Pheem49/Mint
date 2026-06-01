use std::{fs, path::Path};

use anyhow::{Context, Result, bail};
use mint_core::{LearnedSkill, MemoryStore};

const MAX_SKILL_BYTES: u64 = 256 * 1024;
const MAX_CONTEXT_BYTES: usize = 16 * 1024;

pub fn learn(path: &Path) -> Result<LearnedSkill> {
    let path = path
        .canonicalize()
        .with_context(|| format!("unable to resolve skill file {}", path.display()))?;
    let metadata = fs::metadata(&path)?;
    if !metadata.is_file() {
        bail!("skill path is not a file: {}", path.display());
    }
    if metadata.len() > MAX_SKILL_BYTES {
        bail!(
            "skill file is too large ({} bytes); limit is {MAX_SKILL_BYTES} bytes",
            metadata.len()
        );
    }
    if !path
        .extension()
        .and_then(|value| value.to_str())
        .is_some_and(|extension| matches!(extension.to_ascii_lowercase().as_str(), "md" | "txt"))
    {
        bail!("Mint learn supports .md and .txt files only");
    }
    let content = fs::read_to_string(&path)?;
    let name = path
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("skill");
    Ok(MemoryStore::open_default()?.add_learned_skill(name, &path.to_string_lossy(), &content)?)
}

pub fn context() -> Result<String> {
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

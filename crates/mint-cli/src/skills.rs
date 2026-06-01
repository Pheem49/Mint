use std::path::Path;

use anyhow::Result;
use mint_core::{LearnedSkill, learn_skill, learned_skills_context};

pub fn learn(path: &Path) -> Result<LearnedSkill> {
    Ok(learn_skill(path)?)
}

pub fn context() -> Result<String> {
    Ok(learned_skills_context()?)
}

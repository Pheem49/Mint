use std::path::Path;

use anyhow::Result;
use mint_core::{LearnedSkill, learn_skill};

pub fn learn(path: &Path) -> Result<LearnedSkill> {
    Ok(learn_skill(path)?)
}

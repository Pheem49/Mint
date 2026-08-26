use std::path::{Path, PathBuf};

use anyhow::Result;
use mint_core::{LearnedSkill, learn_skill};

pub fn learn(path: &Path) -> Result<LearnedSkill> {
    Ok(learn_skill(path)?)
}

/// Shared by `/skill add` (interactive chat) and `mint skills add` (CLI) —
/// installs `source` as a skill and returns a ready-to-print outcome message.
/// `Ok` = success, `Err` = what went wrong; neither carries ANSI color, so
/// each caller wraps the string in its own color constants.
///
/// `extra_args` is only meaningful for the remote/`npx skills` path — it's
/// forwarded verbatim (e.g. `["--skill", "find-skills"]` to install just one
/// skill out of a multi-skill repo, or `["--agent", "cursor"]` to target a
/// different agent's directory instead of the default). Ignored for local
/// paths, which don't go through npx at all.
pub fn add(
    source: &str,
    extra_args: &[&str],
    current_dir: &Path,
) -> std::result::Result<String, String> {
    let source_path = PathBuf::from(source);
    let source_path = if source_path.is_absolute() {
        source_path
    } else {
        current_dir.join(&source_path)
    };

    if source_path.exists() {
        let home = dirs::home_dir().ok_or("Unable to resolve home directory for Global config.")?;
        let global_skills_path = home.join(".config").join("mint").join("mint-skills");
        if !global_skills_path.exists() {
            let _ = std::fs::create_dir_all(&global_skills_path);
        }

        let name = source_path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("skill");
        let dest_path = global_skills_path.join(name);
        let copy_res = if source_path.is_dir() {
            crate::interactive::copy_dir_all(&source_path, &dest_path)
        } else {
            std::fs::copy(&source_path, &dest_path).map(|_| ())
        };

        copy_res
            .map(|()| {
                format!(
                    "Skill successfully copied to Global config: {}",
                    dest_path.display()
                )
            })
            .map_err(|e| format!("Failed to copy skill to Global: {e}"))
    } else {
        // Not a local path — try resolving it as a remote source through the
        // community `npx skills` CLI (vercel-labs/skills on npm), which
        // already knows how to pull a SKILL.md from a GitHub shorthand
        // (owner/repo), a full GitHub/GitLab/git URL, or a direct download
        // URL. Targeting its "codex" agent isn't about Codex specifically —
        // that agent's project-scoped install path, `.agents/skills/`, is
        // the exact directory Mint's own skill loader already scans, so
        // nothing else needs to change for Mint to pick it up afterward.
        // Respect an explicit --agent/-a or -y/--yes the caller already
        // passed in `extra_args` instead of blindly appending our own
        // defaults on top — only fill in what's missing.
        let has_agent = extra_args.iter().any(|a| *a == "-a" || *a == "--agent");
        let has_yes = extra_args.iter().any(|a| *a == "-y" || *a == "--yes");

        let mut cmd_args = vec!["skills", "add", source];
        cmd_args.extend_from_slice(extra_args);
        if !has_agent {
            cmd_args.push("--agent");
            cmd_args.push("codex");
        }
        if !has_yes {
            cmd_args.push("-y");
        }

        let status = std::process::Command::new("npx")
            .args(&cmd_args)
            .current_dir(current_dir)
            .status();

        match status {
            Ok(s) if s.success() => Ok(format!(
                "Installed into {}/.agents/skills/ — Mint picks it up from there \
                 automatically, no extra step needed.",
                current_dir.display()
            )),
            Ok(_) => Err("`npx skills add` failed — check the source and try again.".into()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Err(
                "`npx` not found — install Node.js to add skills from a GitHub repo or URL: \
                 https://nodejs.org"
                    .into(),
            ),
            Err(e) => Err(format!("Failed to run npx skills: {e}")),
        }
    }
}

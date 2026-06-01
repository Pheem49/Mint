use std::process::Command;

use anyhow::{Context, Result, bail};

const PACKAGE: &str = "@pheem49/mint";

pub fn run(check_only: bool, dry_run: bool, approved: bool) -> Result<()> {
    let current = env!("CARGO_PKG_VERSION");
    let output = Command::new(npm())
        .args(["view", PACKAGE, "version", "--json"])
        .output()
        .context("unable to run npm update check")?;
    if !output.status.success() {
        bail!(
            "npm update check failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    let latest = String::from_utf8_lossy(&output.stdout)
        .trim()
        .trim_matches('"')
        .to_owned();
    if compare_versions(current, &latest) >= 0 {
        println!("Mint is already up to date ({current}).");
        return Ok(());
    }
    println!("Mint {latest} is available. Current version: {current}.");
    if check_only {
        return Ok(());
    }
    if !approved {
        bail!("update installation requires --approve");
    }
    let mut command = Command::new(npm());
    command.args(["install", "-g", &format!("{PACKAGE}@latest")]);
    if dry_run {
        command.arg("--dry-run");
    }
    let status = command
        .status()
        .context("unable to run npm global update")?;
    if !status.success() {
        bail!("npm global update failed with status {status}");
    }
    println!(
        "{}",
        if dry_run {
            "Update dry run complete."
        } else {
            "Mint updated. Restart mint to use the new version."
        }
    );
    Ok(())
}

fn npm() -> &'static str {
    if cfg!(target_os = "windows") {
        "npm.cmd"
    } else {
        "npm"
    }
}

fn compare_versions(left: &str, right: &str) -> i8 {
    let parse = |value: &str| {
        value
            .trim_start_matches('v')
            .split('-')
            .next()
            .unwrap_or_default()
            .split('.')
            .map(|part| part.parse::<u64>().unwrap_or(0))
            .collect::<Vec<_>>()
    };
    let left = parse(left);
    let right = parse(right);
    for index in 0..left.len().max(right.len()).max(3) {
        match left
            .get(index)
            .unwrap_or(&0)
            .cmp(right.get(index).unwrap_or(&0))
        {
            std::cmp::Ordering::Greater => return 1,
            std::cmp::Ordering::Less => return -1,
            std::cmp::Ordering::Equal => {}
        }
    }
    0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compares_semantic_versions() {
        assert_eq!(compare_versions("1.5.4", "1.6.0"), -1);
        assert_eq!(compare_versions("v2.0.0-alpha.1", "2.0.0"), 0);
    }
}

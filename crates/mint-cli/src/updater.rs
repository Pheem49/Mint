use std::process::Command;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

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
    write_cache(&latest);
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

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct UpdateCache {
    last_checked: u64,
    latest_version: String,
}

fn cache_path() -> Option<PathBuf> {
    #[cfg(test)]
    {
        Some(std::env::temp_dir().join("update-cache.json"))
    }
    #[cfg(not(test))]
    {
        dirs::config_dir().map(|dir| dir.join("mint").join("update-cache.json"))
    }
}

fn get_current_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn write_cache(latest_version: &str) -> Option<()> {
    let path = cache_path()?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).ok()?;
    }
    let cache = UpdateCache {
        last_checked: get_current_timestamp(),
        latest_version: latest_version.to_owned(),
    };
    let json = serde_json::to_string_pretty(&cache).ok()?;
    std::fs::write(path, json).ok()?;
    Some(())
}

pub fn get_cached_update_notice() -> Option<(String, String)> {
    let path = cache_path()?;
    if !path.exists() {
        return None;
    }
    let data = std::fs::read_to_string(path).ok()?;
    let cache: UpdateCache = serde_json::from_str(&data).ok()?;
    let current = env!("CARGO_PKG_VERSION");
    if compare_versions(current, &cache.latest_version) < 0 {
        Some((current.to_owned(), cache.latest_version))
    } else {
        None
    }
}

pub fn should_check_for_update() -> bool {
    let path = match cache_path() {
        Some(p) => p,
        None => return false,
    };
    if !path.exists() {
        return true;
    }
    let data = match std::fs::read_to_string(path) {
        Ok(d) => d,
        Err(_) => return true,
    };
    let cache: UpdateCache = match serde_json::from_str(&data) {
        Ok(c) => c,
        Err(_) => return true,
    };
    let now = get_current_timestamp();
    // Check once every 24 hours (86400 seconds)
    now.saturating_sub(cache.last_checked) > 86400
}

pub fn check_for_update_quietly() -> Option<(String, String)> {
    let current = env!("CARGO_PKG_VERSION");
    let output = Command::new(npm())
        .args(["view", PACKAGE, "version", "--json"])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let latest = String::from_utf8_lossy(&output.stdout)
        .trim()
        .trim_matches('"')
        .to_owned();
    write_cache(&latest);
    if compare_versions(current, &latest) < 0 {
        Some((current.to_owned(), latest))
    } else {
        None
    }
}

pub fn print_update_notice(current: &str, latest: &str) {
    let command_msg = "Run sh -c 'curl -fsSL https://raw.githubusercontent.com/Pheem49/Mint/main/install.sh | MINT_NON_INTERACTIVE=1 sh' to update.";
    let notes_label = "See full release notes:";
    let notes_url = "https://github.com/Pheem49/Mint/releases/latest";

    // Text lengths (including 1 leading space)
    let title_clean_len = 1 + 3 + format!("Update available! {} -> {}", current, latest).chars().count();
    let command_msg_len = 1 + command_msg.len();
    let notes_label_len = 1 + notes_label.len();
    let notes_url_len = 1 + notes_url.len();

    let max_len = command_msg_len
        .max(title_clean_len)
        .max(notes_label_len)
        .max(notes_url_len) + 1; // plus 1 for trailing space before right border

    let border = "─".repeat(max_len);
    println!("\x1b[33m╭{}╮\x1b[0m", border);
    
    // Line 1: Title
    let title_display = format!(" ✨ Update available! \x1b[1;32m{}\x1b[0;33m -> \x1b[1;32m{}\x1b[0;33m", current, latest);
    let padding1 = max_len - title_clean_len;
    println!("\x1b[33m│\x1b[0m{}{}\x1b[33m│\x1b[0m", title_display, " ".repeat(padding1));
    
    // Line 2: Command
    let padding2 = max_len - command_msg_len;
    println!(
        "\x1b[33m│\x1b[0m \x1b[37m{}\x1b[0m{}\x1b[33m│\x1b[0m",
        command_msg,
        " ".repeat(padding2)
    );

    // Line 3: Empty separator
    println!("\x1b[33m│\x1b[0m{}\x1b[33m│\x1b[0m", " ".repeat(max_len));

    // Line 4: Notes label
    let padding4 = max_len - notes_label_len;
    println!(
        "\x1b[33m│\x1b[0m \x1b[90m{}\x1b[0m{}\x1b[33m│\x1b[0m",
        notes_label,
        " ".repeat(padding4)
    );

    // Line 5: Notes URL
    let padding5 = max_len - notes_url_len;
    println!(
        "\x1b[33m│\x1b[0m \x1b[36m{}\x1b[0m{}\x1b[33m│\x1b[0m",
        notes_url,
        " ".repeat(padding5)
    );
    
    println!("\x1b[33m╰{}╯\x1b[0m\n", border);
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

    #[test]
    fn test_write_and_read_cache() {
        let path = cache_path().unwrap();
        if path.exists() {
            let _ = std::fs::remove_file(&path);
        }
        assert!(get_cached_update_notice().is_none());
        assert!(write_cache("99.9.9").is_some());
        let notice = get_cached_update_notice();
        assert!(notice.is_some());
        let (_current, latest) = notice.unwrap();
        assert_eq!(latest, "99.9.9");
        let _ = std::fs::remove_file(&path);
    }
}

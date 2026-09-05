use crate::{DIM, MINT, RESET, WARN};
use anyhow::Result;
use std::fs;
use std::path::Path;

pub fn open_system_handler(target: &str) -> Result<()> {
    #[cfg(target_os = "windows")]
    {
        std::process::Command::new("cmd")
            .args(&["/c", "start", "", target])
            .spawn()?;
    }
    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open").arg(target).spawn()?;
    }
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    {
        std::process::Command::new("xdg-open").arg(target).spawn()?;
    }
    Ok(())
}

pub fn launch_desktop_app(name: &str) -> Result<()> {
    #[cfg(target_os = "windows")]
    {
        std::process::Command::new("cmd")
            .args(&["/c", "start", "", name])
            .spawn()?;
    }
    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open")
            .args(&["-a", name])
            .spawn()?;
    }
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    {
        if std::process::Command::new(name)
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
            .is_ok()
        {
            return Ok(());
        }
        let lower = name.to_lowercase();
        if std::process::Command::new("gtk-launch")
            .arg(&lower)
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
            .is_ok()
        {
            return Ok(());
        }
        std::process::Command::new("xdg-open").arg(name).spawn()?;
    }
    Ok(())
}

pub fn read_file_content(path: &Path) -> Result<()> {
    let content = fs::read_to_string(path)?;
    println!("{}", content);
    Ok(())
}

pub fn read_folder_content(path: &Path) -> Result<()> {
    let entries = fs::read_dir(path)?;
    for entry in entries {
        let entry = entry?;
        let file_name = entry.file_name();
        let file_name_str = file_name.to_string_lossy();
        let metadata = entry.metadata()?;
        if metadata.is_dir() {
            println!("{MINT}{}/{}", file_name_str, RESET);
        } else {
            println!("{}", file_name_str);
        }
    }
    Ok(())
}

pub fn print_shell_output(output: &mint_core::ShellOutput) {
    if !output.stdout.is_empty() {
        print!("{}", output.stdout);
    }
    if !output.stderr.is_empty() {
        eprint!("{}", output.stderr);
    }
    if !output.stdout.ends_with('\n') && !output.stderr.ends_with('\n') {
        println!();
    }
    println!(
        "{DIM}[exit: {} | sandboxed: {}]{RESET}",
        output
            .status
            .map_or_else(|| "unknown".into(), |status| status.to_string()),
        output.sandboxed
    );
    if let Some(warning) = &output.sandbox_warning {
        println!("{WARN}[Warning]{RESET} {warning}");
    }
    println!();
}

pub fn preview_file(path: &Path) -> Result<()> {
    if !path.exists() {
        anyhow::bail!("File not found: {}", path.display());
    }
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();
    let metadata = fs::metadata(path)?;
    let size = metadata.len();

    println!(
        "{MINT}Mint Live Preview{RESET}: {} {DIM}({} bytes){RESET}",
        path.display(),
        size
    );

    match ext.as_str() {
        "html" | "htm" | "svg" => {
            let canonical = path.canonicalize()?;
            let url = format!("file://{}", canonical.display());
            println!("{DIM}Opening in default browser/viewer: {RESET}{url}");
            open_system_handler(&url)?;
        }
        "md" | "markdown" => {
            let content = fs::read_to_string(path)?;
            println!("\n{DIM}--- Markdown Preview ---{RESET}\n");
            println!("{}", content);
            println!("\n{DIM}--- End Preview ---{RESET}");
        }
        _ => {
            let content = fs::read_to_string(path)?;
            let lines: Vec<&str> = content.lines().take(50).collect();
            println!("\n{DIM}--- File Preview (first 50 lines) ---{RESET}\n");
            for (idx, line) in lines.iter().enumerate() {
                println!("{DIM}{:4} |{RESET} {}", idx + 1, line);
            }
            if content.lines().count() > 50 {
                println!(
                    "{DIM}... and {} more lines{RESET}",
                    content.lines().count() - 50
                );
            }
        }
    }
    Ok(())
}

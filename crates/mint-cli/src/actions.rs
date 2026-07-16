use anyhow::Result;
use std::fs;
use std::path::{Path, PathBuf};
use mint_core::MintConfig;
use crate::{MINT, RESET, DIM, ERROR, WARN};
use crate::confirm; // Will be moved to interactive later, or crate::interactive::confirm

pub struct ActionStreamFilter {
    buffer: String,
    in_action: bool,
    action_text: String,
    actions: Vec<String>,
}

impl ActionStreamFilter {
    pub fn new() -> Self {
        Self {
            buffer: String::new(),
            in_action: false,
            action_text: String::new(),
            actions: Vec::new(),
        }
    }

    pub fn process_chunk(&mut self, chunk: &str, mut print_fn: impl FnMut(&str)) {
        for c in chunk.chars() {
            if self.in_action {
                if c == ']' {
                    self.in_action = false;
                    self.actions.push(self.action_text.clone());
                    self.action_text.clear();
                } else {
                    self.action_text.push(c);
                }
            } else {
                self.buffer.push(c);
                let action_prefix = "[ACTION:";
                if action_prefix.starts_with(&self.buffer) {
                    if self.buffer == action_prefix {
                        self.in_action = true;
                        self.buffer.clear();
                    }
                } else {
                    if let Some(pos) = self.buffer.find('[') {
                        print_fn(&self.buffer[..pos]);
                        let new_buf = self.buffer[pos..].to_string();
                        self.buffer = new_buf;
                        if !action_prefix.starts_with(&self.buffer) {
                            print_fn(&self.buffer);
                            self.buffer.clear();
                        }
                    } else {
                        print_fn(&self.buffer);
                        self.buffer.clear();
                    }
                }
            }
        }
    }

    pub fn finalize(&mut self, mut print_fn: impl FnMut(&str)) -> Vec<String> {
        if !self.buffer.is_empty() {
            print_fn(&self.buffer);
            self.buffer.clear();
        }
        if self.in_action {
            print_fn(&format!("[ACTION:{}", self.action_text));
            self.action_text.clear();
            self.in_action = false;
        }
        std::mem::take(&mut self.actions)
    }
}

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

pub fn confirm_shell_execution() -> Result<bool> {
    confirm("Approve local shell execution? [y/N]")
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
        "{DIM}[exit: {} | sandboxed: {}]{RESET}\n",
        output
            .status
            .map_or_else(|| "unknown".into(), |status| status.to_string()),
        output.sandboxed
    );
}

pub fn execute_action(action: &str, config: &MintConfig) -> Result<()> {
    let trimmed = action.trim();
    if let Some((cmd, args)) = trimmed.split_once(' ') {
        let cmd = cmd.trim();
        let args = args.trim();
        match cmd {
            "open" => {
                println!("{WARN}System action:{RESET} opening {args}...\n");
                open_system_handler(args)?;
            }
            "open-app" => {
                println!("{WARN}System action:{RESET} launching app {args}...\n");
                launch_desktop_app(args)?;
            }
            "read-file" => {
                println!("{WARN}System action:{RESET} reading file {args}...\n");
                let path = PathBuf::from(args);
                read_file_content(&path)?;
            }
            "read-folder" => {
                println!("{WARN}System action:{RESET} reading folder {args}...\n");
                let path = PathBuf::from(args);
                read_folder_content(&path)?;
            }
            "run-shell" => {
                println!("{WARN}System action:{RESET} run local shell command");
                println!("  {args}");
                if confirm_shell_execution()? {
                    let output = mint_core::run_shell_command(args, &std::env::current_dir()?, true, config)?;
                    print_shell_output(&output);
                } else {
                    println!("Shell command cancelled.\n");
                }
            }
            _ => {
                println!("{ERROR}Unknown system action:{RESET} {cmd} with args {args}\n");
            }
        }
    } else {
        match trimmed {
            "read-folder" => {
                println!("{WARN}System action:{RESET} reading folder . ...\n");
                read_folder_content(&PathBuf::from("."))?;
            }
            _ => {
                println!("{ERROR}Invalid action format:{RESET} {trimmed}\n");
            }
        }
    }
    Ok(())
}

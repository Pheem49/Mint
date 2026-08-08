//! Best-effort append-only log for browser automation actions. `mint auto`
//! tails `browser-automation.log` in the CLI to show live progress in the
//! terminal, so every public action in this module reports through here.

pub(super) fn log_action(action: &str, details: &str) {
    if let Some(config_dir) = dirs::config_dir() {
        let log_dir = config_dir.join("mint");
        let log_file = log_dir.join("browser-automation.log");
        let _ = std::fs::create_dir_all(&log_dir);
        if let Ok(mut file) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&log_file)
        {
            use std::io::Write;
            let now = chrono::Local::now().format("%Y-%m-%d %H:%M:%S");
            let _ = writeln!(file, "[{}] [{}] {}", now, action, details);
        }
    }
}

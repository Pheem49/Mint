//! Headless Chrome through `run_shell`, Codex style: the model writes an ordinary
//! `google-chrome --headless=new --screenshot=… URL` command and Mint makes it
//! safe and repeatable instead of exposing a browser tool surface.
//!
//! Interface: [`prepare`] (and [`prepare_with`] for tests). Everything else is
//! hidden: recognising a Chrome invocation, injecting the flags that make
//! headless runs reliable, giving each run a throw-away profile, bounding its
//! runtime, and computing a [`CaptureKey`] so the caller can refuse to recapture
//! the same page with the same options in one turn.
use crate::MintConfig;
use serde_json::Value;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

/// Wall-clock limit applied to a single Chrome run on Linux.
const RUN_TIMEOUT_SECS: u64 = 45;
/// Virtual time Chrome may spend letting a SPA settle before capturing.
const VIRTUAL_TIME_BUDGET_MS: u32 = 8000;
/// Throw-away profiles older than this are removed on the next `prepare`.
const STALE_PROFILE_AGE: Duration = Duration::from_secs(60 * 60);

const CHROME_PROGRAMS: &[&str] = &[
    "google-chrome",
    "google-chrome-stable",
    "chromium",
    "chromium-browser",
    "chrome",
    "microsoft-edge",
    "msedge",
];

/// Identity of "this page captured this way". The output path is deliberately
/// not part of it: recapturing the same URL to a new filename is still a repeat.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CaptureKey(String);

impl CaptureKey {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// A Chrome command that is ready to run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChromeRun {
    /// The hardened command line to execute (and to show the user for approval).
    pub command: String,
    /// Present when the command targets exactly one URL.
    pub key: Option<CaptureKey>,
    /// File the command writes (`--screenshot` / `--print-to-pdf`), as the model wrote it.
    pub output: Option<String>,
}

/// Host facts that change which flags are needed. Injected so tests are deterministic.
#[derive(Debug, Clone)]
pub struct HostEnv {
    pub no_sandbox: bool,
    pub linux_timeout: bool,
    pub profile_root: PathBuf,
}

impl HostEnv {
    pub fn detect() -> Self {
        Self {
            no_sandbox: cfg!(target_os = "linux") && (is_root() || Path::new("/.dockerenv").exists()),
            linux_timeout: cfg!(target_os = "linux"),
            profile_root: std::env::temp_dir().join("mint-headless"),
        }
    }
}

#[cfg(unix)]
fn is_root() -> bool {
    // SAFETY-free check: `id -u` equivalent via the effective uid from /proc.
    std::fs::read_to_string("/proc/self/status")
        .ok()
        .and_then(|status| {
            status
                .lines()
                .find(|line| line.starts_with("Uid:"))
                .and_then(|line| line.split_whitespace().nth(2).map(str::to_owned))
        })
        .is_some_and(|euid| euid == "0")
}

#[cfg(not(unix))]
fn is_root() -> bool {
    false
}

/// Hardens a shell command if it invokes Chrome; `None` for anything else.
pub fn prepare(command: &str, config: &MintConfig) -> Option<ChromeRun> {
    prepare_with(command, config, &HostEnv::detect())
}

pub fn prepare_with(command: &str, config: &MintConfig, host: &HostEnv) -> Option<ChromeRun> {
    let tokens = tokenize(command);
    let chrome = find_chrome_token(&tokens, configured_program(config).as_deref())?;
    let args: Vec<&str> = tokens[chrome + 1..]
        .iter()
        .map(|t| t.text)
        .take_while(|t| !is_operator(t))
        .collect();

    let has = |name: &str| {
        args.iter()
            .any(|a| *a == name || a.starts_with(&format!("{name}=")))
    };
    let flag_value = |name: &str| {
        args.iter()
            .find_map(|a| a.strip_prefix(&format!("{name}=")))
            .map(|value| unquote(value).to_owned())
    };

    let mode = if has("--dump-dom") {
        "dom"
    } else if has("--screenshot") {
        "screenshot"
    } else if has("--print-to-pdf") {
        "pdf"
    } else {
        "load"
    };
    let captures = matches!(mode, "dom" | "screenshot");

    let mut inject: Vec<String> = Vec::new();
    if !has("--headless") {
        inject.push("--headless=new".into());
    }
    if !has("--disable-gpu") {
        inject.push("--disable-gpu".into());
    }
    if cfg!(target_os = "linux") && !has("--disable-dev-shm-usage") {
        inject.push("--disable-dev-shm-usage".into());
    }
    if host.no_sandbox && !has("--no-sandbox") {
        inject.push("--no-sandbox".into());
        inject.push("--disable-setuid-sandbox".into());
    }
    if !has("--user-data-dir") {
        sweep_stale_profiles(&host.profile_root, STALE_PROFILE_AGE);
        let profile = host.profile_root.join(uuid::Uuid::new_v4().to_string());
        let _ = std::fs::create_dir_all(&profile);
        inject.push(format!("--user-data-dir={}", quote(&profile.to_string_lossy())));
    }
    for flag in ["--no-first-run", "--no-default-browser-check"] {
        if !has(flag) {
            inject.push(flag.into());
        }
    }
    let window_size = flag_value("--window-size");
    if mode == "screenshot" && window_size.is_none() {
        let (width, height) = viewport(config);
        inject.push(format!("--window-size={width},{height}"));
    }
    if captures && !has("--virtual-time-budget") {
        inject.push(format!("--virtual-time-budget={VIRTUAL_TIME_BUDGET_MS}"));
    }

    // `timeout google-chrome …` or `timeout 10 google-chrome …` (flags such as -s KILL are not parsed).
    let already_bounded = (chrome >= 1 && tokens[chrome - 1].text == "timeout")
        || (chrome >= 2 && tokens[chrome - 2].text == "timeout");
    let mut prefix = String::new();
    if host.linux_timeout && !already_bounded {
        prefix = format!("timeout {RUN_TIMEOUT_SECS} ");
    }

    let program = &tokens[chrome];
    let mut out = String::with_capacity(command.len() + 160);
    out.push_str(&command[..program.start]);
    out.push_str(&prefix);
    out.push_str(&command[program.start..program.end]);
    for flag in &inject {
        out.push(' ');
        out.push_str(flag);
    }
    out.push_str(&command[program.end..]);

    let output = match mode {
        "screenshot" => Some(flag_value("--screenshot").unwrap_or_else(|| "screenshot.png".into())),
        "pdf" => flag_value("--print-to-pdf"),
        _ => None,
    }
    .filter(|path| !path.is_empty());

    let urls: Vec<&str> = args
        .iter()
        .map(|a| unquote(a))
        .filter(|a| a.starts_with("http://") || a.starts_with("https://"))
        .collect();
    let key = (urls.len() == 1).then(|| {
        CaptureKey(format!(
            "{mode}|{}|{}",
            window_size.unwrap_or_else(|| "default".into()),
            normalize_url(urls[0])
        ))
    });

    Some(ChromeRun { command: out, key, output })
}

fn configured_program(config: &MintConfig) -> Option<String> {
    config
        .extra
        .get("browserExecutable")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(|s| base_name(s).to_owned())
}

fn viewport(config: &MintConfig) -> (u64, u64) {
    let get = |key: &str, default: u64, min: u64, max: u64| {
        config
            .extra
            .get(key)
            .and_then(Value::as_u64)
            .unwrap_or(default)
            .clamp(min, max)
    };
    (
        get("browserViewportWidth", 1440, 320, 7680),
        get("browserViewportHeight", 900, 240, 4320),
    )
}

struct Token<'a> {
    text: &'a str,
    start: usize,
    end: usize,
}

fn tokenize(command: &str) -> Vec<Token<'_>> {
    let mut tokens = Vec::new();
    let mut start = None;
    let mut quote: Option<char> = None;
    for (index, ch) in command.char_indices() {
        match (start, quote) {
            (None, _) if ch.is_whitespace() => {}
            (None, _) => {
                start = Some(index);
                if ch == '"' || ch == '\'' {
                    quote = Some(ch);
                }
            }
            (Some(_), Some(q)) => {
                if ch == q {
                    quote = None;
                }
            }
            (Some(s), None) => {
                if ch.is_whitespace() {
                    tokens.push(Token { text: &command[s..index], start: s, end: index });
                    start = None;
                } else if ch == '"' || ch == '\'' {
                    quote = Some(ch);
                }
            }
        }
    }
    if let Some(s) = start {
        tokens.push(Token { text: &command[s..], start: s, end: command.len() });
    }
    tokens
}

fn is_operator(token: &str) -> bool {
    matches!(token, "|" | "||" | "&&" | ";" | "&" | ">" | ">>" | "<" | "2>" | "2>&1")
        || token.ends_with(';')
}

/// A Chrome *invocation*, not a mention: the program sits in command position.
fn find_chrome_token(tokens: &[Token<'_>], configured: Option<&str>) -> Option<usize> {
    tokens.iter().enumerate().position(|(i, token)| {
        let name = base_name(unquote(token.text)).to_ascii_lowercase();
        let name = name.trim_end_matches(".exe");
        let known = CHROME_PROGRAMS.contains(&name)
            || configured.is_some_and(|c| c.eq_ignore_ascii_case(name));
        known && in_command_position(tokens, i)
    })
}

fn in_command_position(tokens: &[Token<'_>], index: usize) -> bool {
    let mut i = index;
    while i > 0 {
        let previous = tokens[i - 1].text;
        if previous == "timeout" || previous.chars().all(|c| c.is_ascii_digit() || c == '.') && i >= 2 && tokens[i - 2].text == "timeout" {
            i -= 1;
            continue;
        }
        // Environment assignment (`FOO=bar google-chrome …`).
        if !previous.starts_with('-') && previous.contains('=') && !previous.contains('/') {
            i -= 1;
            continue;
        }
        return matches!(previous, "&&" | "||" | "|" | ";" | "&" | "(" | "then" | "do")
            || previous.ends_with(';');
    }
    true
}

fn base_name(path: &str) -> &str {
    path.rsplit(['/', '\\']).next().unwrap_or(path)
}

fn unquote(token: &str) -> &str {
    token
        .trim_matches(|c| c == '"' || c == '\'')
}

fn quote(value: &str) -> String {
    if value.chars().any(char::is_whitespace) {
        format!("'{}'", value.replace('\'', "'\\''"))
    } else {
        value.to_owned()
    }
}

fn normalize_url(url: &str) -> String {
    let without_fragment = url.split('#').next().unwrap_or(url);
    let (scheme, rest) = without_fragment
        .split_once("://")
        .unwrap_or(("https", without_fragment));
    let (authority, path) = match rest.find(['/', '?']) {
        Some(i) => rest.split_at(i),
        None => (rest, ""),
    };
    let path = if path == "/" { "" } else { path.trim_end_matches('/') };
    format!(
        "{}://{}{}",
        scheme.to_ascii_lowercase(),
        authority.to_ascii_lowercase(),
        path
    )
}

/// Removes throw-away profiles left behind by earlier runs.
pub fn sweep_stale_profiles(root: &Path, max_age: Duration) {
    let Ok(entries) = std::fs::read_dir(root) else {
        return;
    };
    let now = SystemTime::now();
    for entry in entries.flatten() {
        let old = entry
            .metadata()
            .and_then(|m| m.modified())
            .ok()
            .and_then(|modified| now.duration_since(modified).ok())
            .is_some_and(|age| age >= max_age);
        if old {
            let _ = std::fs::remove_dir_all(entry.path());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn host() -> HostEnv {
        HostEnv {
            no_sandbox: false,
            linux_timeout: true,
            profile_root: std::env::temp_dir()
                .join(format!("mint-headless-test-{}", uuid::Uuid::new_v4())),
        }
    }

    fn run(command: &str) -> ChromeRun {
        prepare_with(command, &MintConfig::default(), &host()).expect("chrome command")
    }

    fn count(haystack: &str, needle: &str) -> usize {
        haystack.matches(needle).count()
    }

    #[test]
    fn bare_screenshot_command_gets_the_reliability_flags() {
        let out = run("google-chrome --screenshot=/tmp/a.png https://example.com").command;
        assert!(out.starts_with("timeout 45 google-chrome "), "{out}");
        for flag in [
            "--headless=new",
            "--disable-gpu",
            "--disable-dev-shm-usage",
            "--user-data-dir=",
            "--window-size=1440,900",
            "--virtual-time-budget=8000",
            "--no-first-run",
        ] {
            assert!(out.contains(flag), "missing {flag} in {out}");
        }
        assert!(out.ends_with("--screenshot=/tmp/a.png https://example.com"), "{out}");
    }

    #[test]
    fn flags_the_model_already_chose_are_not_duplicated_or_overridden() {
        let out = run(
            "google-chrome --headless --user-data-dir=/x --window-size=800,600 \
             --virtual-time-budget=100 --screenshot=/tmp/a.png https://example.com",
        )
        .command;
        assert_eq!(count(&out, "--headless"), 1, "{out}");
        assert_eq!(count(&out, "--user-data-dir"), 1, "{out}");
        assert_eq!(count(&out, "--window-size"), 1, "{out}");
        assert_eq!(count(&out, "--virtual-time-budget"), 1, "{out}");
        assert!(!out.contains("--headless=new"), "{out}");
    }

    #[test]
    fn mentions_of_chrome_are_not_chrome_invocations() {
        let config = MintConfig::default();
        let env = host();
        for command in ["ls -la", "echo google-chrome", "grep chrome notes.txt", "cargo test chromium"] {
            assert!(prepare_with(command, &config, &env).is_none(), "{command}");
        }
    }

    #[test]
    fn chrome_after_cd_or_env_assignment_is_recognised_and_redirects_survive() {
        let out = run("cd /tmp && LANG=C google-chrome --dump-dom https://example.com > out.html").command;
        assert!(out.starts_with("cd /tmp && LANG=C timeout 45 google-chrome "), "{out}");
        assert!(out.ends_with("--dump-dom https://example.com > out.html"), "{out}");
    }

    #[test]
    fn an_existing_timeout_wrapper_is_not_doubled() {
        let out = run("timeout 10 google-chrome --dump-dom https://example.com").command;
        assert_eq!(count(&out, "timeout"), 1, "{out}");
    }

    #[test]
    fn sandbox_is_only_disabled_when_the_host_requires_it() {
        let config = MintConfig::default();
        let mut env = host();
        let plain = prepare_with("chromium --dump-dom https://a.io", &config, &env).unwrap();
        assert!(!plain.command.contains("--no-sandbox"));
        env.no_sandbox = true;
        let sandboxless = prepare_with("chromium --dump-dom https://a.io", &config, &env).unwrap();
        assert!(sandboxless.command.contains("--no-sandbox"));
    }

    #[test]
    fn plain_load_without_capture_gets_no_virtual_time_budget() {
        let out = run("google-chrome https://example.com").command;
        assert!(!out.contains("--virtual-time-budget"), "{out}");
    }

    #[test]
    fn same_page_and_options_share_a_key_regardless_of_output_path_or_url_spelling() {
        let a = run("google-chrome --screenshot=/tmp/a.png https://Example.com/shop/#top");
        let b = run("google-chrome --screenshot=/tmp/b.png 'https://example.com/shop'");
        assert!(a.key.is_some());
        assert_eq!(a.key, b.key);
        assert_eq!(a.output.as_deref(), Some("/tmp/a.png"));
    }

    #[test]
    fn different_mode_viewport_or_page_gets_a_different_key() {
        let base = run("google-chrome --screenshot=/tmp/a.png https://example.com/a").key;
        assert_ne!(base, run("google-chrome --dump-dom https://example.com/a").key);
        assert_ne!(
            base,
            run("google-chrome --window-size=800,600 --screenshot=/tmp/a.png https://example.com/a").key
        );
        assert_ne!(base, run("google-chrome --screenshot=/tmp/a.png https://example.com/b").key);
    }

    #[test]
    fn commands_without_exactly_one_url_have_no_key() {
        assert!(run("google-chrome --dump-dom").key.is_none());
        assert!(run("google-chrome --dump-dom https://a.io https://b.io").key.is_none());
    }

    #[test]
    fn bare_screenshot_flag_reports_chromes_default_output_file() {
        let out = run("google-chrome --screenshot https://example.com");
        assert_eq!(out.output.as_deref(), Some("screenshot.png"));
    }

    #[test]
    fn each_run_gets_its_own_profile_and_stale_ones_are_swept() {
        let config = MintConfig::default();
        let env = host();
        let first = prepare_with("chromium --dump-dom https://a.io", &config, &env).unwrap();
        let second = prepare_with("chromium --dump-dom https://a.io", &config, &env).unwrap();
        assert_ne!(first.command, second.command);
        assert_eq!(std::fs::read_dir(&env.profile_root).unwrap().count(), 2);
        sweep_stale_profiles(&env.profile_root, Duration::ZERO);
        assert_eq!(std::fs::read_dir(&env.profile_root).unwrap().count(), 0);
        let _ = std::fs::remove_dir_all(&env.profile_root);
    }
}

use anyhow::Result;
use mint_core::{HookEntry, HookEvent, add_hook, clear_hooks, list_hooks, load_config, remove_hook};

pub fn list() -> Result<Vec<HookEntry>> {
    Ok(list_hooks(&load_config()?))
}

pub fn add(event: HookEvent, matcher: &str, command: &str, timeout_secs: Option<u64>) -> Result<()> {
    Ok(add_hook(event, matcher, command, timeout_secs)?)
}

pub fn remove(index: usize) -> Result<bool> {
    Ok(remove_hook(index)?)
}

pub fn clear() -> Result<()> {
    Ok(clear_hooks()?)
}

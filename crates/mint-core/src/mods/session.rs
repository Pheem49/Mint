use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    path::Path,
    sync::{LazyLock, Mutex},
};

#[derive(Clone, Default)]
struct Session {
    hot_reload: bool,
    usage: Option<Value>,
    viewport: Viewport,
    revisions: BTreeMap<String, (String, String)>,
}
static SESSIONS: LazyLock<Mutex<BTreeMap<String, Session>>> =
    LazyLock::new(|| Mutex::new(BTreeMap::new()));

#[derive(Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Viewport {
    pub surface: Option<String>,
    pub columns: Option<u16>,
    pub rows: Option<u16>,
    pub width_px: Option<u32>,
    pub height_px: Option<u32>,
    #[serde(default)]
    pub side_panel_open: bool,
}
pub fn set_hot_reload(root: &Path, chat: &str, enabled: bool) -> Result<(), String> {
    SESSIONS
        .lock()
        .unwrap()
        .entry(super::manager::key(root, chat, "")?)
        .or_default()
        .hot_reload = enabled;
    Ok(())
}
pub fn session_snapshot(root: &Path, chat: &str) -> Result<Value, String> {
    let sessions = SESSIONS.lock().unwrap();
    let current = sessions
        .get(&super::manager::key(root, chat, "")?)
        .cloned()
        .unwrap_or_default();
    Ok(json!({"hotReload":current.hot_reload,"viewport":current.viewport}))
}
pub(super) fn hot_reload(root: &Path, chat: &str) -> bool {
    session_snapshot(root, chat)
        .ok()
        .is_some_and(|v| v["hotReload"] == true)
}
pub(super) fn revision(root: &Path, chat: &str, id: &str, base: &str) -> Option<String> {
    let sessions = SESSIONS.lock().unwrap();
    let s = sessions.get(&super::manager::key(root, chat, "").ok()?)?;
    s.revisions
        .get(id)
        .filter(|(b, _)| b == base)
        .map(|(_, h)| h.clone())
}
pub(super) fn set_revision(
    root: &Path,
    chat: &str,
    id: &str,
    base: &str,
    hash: &str,
) -> Result<(), String> {
    SESSIONS
        .lock()
        .unwrap()
        .entry(super::manager::key(root, chat, "")?)
        .or_default()
        .revisions
        .insert(id.into(), (base.into(), hash.into()));
    Ok(())
}
pub(super) fn clear(root: &Path, chat: &str) {
    if let Ok(k) = super::manager::key(root, chat, "") {
        SESSIONS.lock().unwrap().remove(&k);
    }
}
pub(super) fn clear_matching(marker: &str) {
    SESSIONS
        .lock()
        .unwrap()
        .retain(|key, _| !key.contains(marker));
}
pub fn current_usage(root: &Path, chat: &str) -> Value {
    let current = super::manager::key(root, chat, "").ok().and_then(|k| {
        SESSIONS
            .lock()
            .unwrap()
            .get(&k)
            .and_then(|s| s.usage.clone())
    });
    current
        .or_else(|| {
            super::manager::telemetry_path(root, chat)
                .ok()
                .and_then(|p| std::fs::read(p.with_extension("usage.json")).ok())
                .and_then(|v| serde_json::from_slice(&v).ok())
        })
        .unwrap_or_else(|| super::telemetry::UsageTracker::new(root, chat, 0).snapshot())
}
pub(super) fn store_usage(root: &Path, chat: &str, value: &Value) -> Result<(), String> {
    SESSIONS
        .lock()
        .unwrap()
        .entry(super::manager::key(root, chat, "")?)
        .or_default()
        .usage = Some(value.clone());
    super::manager::atomic_json(
        &super::manager::telemetry_path(root, chat)?.with_extension("usage.json"),
        value,
    )
}
pub fn update_viewport(root: &Path, chat: &str, viewport: Viewport) -> Result<(), String> {
    if !matches!(
        viewport.surface.as_deref(),
        Some("terminal" | "desktop" | "web")
    ) || viewport.columns.is_some_and(|n| n == 0 || n > 10000)
        || viewport.rows.is_some_and(|n| n == 0 || n > 10000)
        || viewport.width_px.is_some_and(|n| n == 0 || n > 100000)
        || viewport.height_px.is_some_and(|n| n == 0 || n > 100000)
    {
        return Err("Invalid mod viewport".into());
    }
    let changed = {
        let mut sessions = SESSIONS.lock().unwrap();
        let s = sessions
            .entry(super::manager::key(root, chat, "")?)
            .or_default();
        let changed = s.viewport != viewport;
        s.viewport = viewport.clone();
        changed
    };
    if changed {
        super::event(
            root,
            chat,
            "ui_resize",
            serde_json::to_value(viewport).map_err(|e| e.to_string())?,
            None,
        )?;
    }
    Ok(())
}

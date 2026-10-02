//! Browser automation for `mint auto`: drives a real Chrome/Chromium/Edge
//! instance over the Chrome DevTools Protocol so the AI can navigate, read,
//! click, type, and screenshot a page.
//!
//! Split by concern:
//! - `session` — run-scoped tab leases, observations, verification, and recovery
//! - `cdp` — the pinned, bounded CDP websocket transport
//! - `lifecycle` — spawning/detecting the automation browser instance
//! - `navigate` — navigation and page-state reading (tabs, text, screenshot)
//! - `interact` — selector-driven click/type
//! - `input` — native mouse/keyboard primitives (`interact` builds on these)
//! - `overlay` — the visible green-aura + cursor overlay injected into the page
//! - `logging` — the `browser-automation.log` tail `mint auto` reads from

mod cdp;
mod evidence;
mod input;
mod interact;
mod lifecycle;
mod logging;
mod navigate;
mod overlay;
mod session;

pub(crate) use lifecycle::BROWSER_TOOLS;
pub use session::{
    BrowserActionResult, BrowserObservation, BrowserSession, execute_action, run_session, with_tab,
};
pub(crate) use session::{
    finish_evidence as session_finish_evidence, last_state as session_last_state, selected_tab,
};

use serde::Serialize;

pub use input::{key_press, mouse_click, mouse_move, type_text_native};
pub use interact::{click, get_element_coordinates, type_text};
pub use lifecycle::{
    enable_browser_tools, ensure_page_open, is_browser_running, spawn_automation_browser,
    spawn_automation_browser_with_url,
};
pub use navigate::{list_tabs, navigate, read_page_text, screenshot};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BrowserTab {
    pub id: String,
    pub title: String,
    pub url: String,
}

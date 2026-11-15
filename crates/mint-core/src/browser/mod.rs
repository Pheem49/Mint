//! Browser automation and headless Chrome integration. Drives Chromium over CDP
//! for bounded workflows (`mint browser run`) and CLI shell execution.

mod cdp;
mod evidence;
pub mod headless;
mod input;
mod interact;
mod navigate;
mod overlay;
pub mod runtime;
mod session;
pub mod workflow;

pub(crate) use session::{capture_metadata, execute_action, run_session, selected_tab};

pub(crate) use input::{key_press, mouse_click, mouse_move};
pub(crate) use interact::{click, get_element_coordinates, type_text};
pub(crate) use navigate::{list_tabs, navigate, read_page_text, screenshot};

use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BrowserTab {
    pub id: String,
    pub title: String,
    pub url: String,
}

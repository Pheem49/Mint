//! Browser automation and headless Chrome integration. Drives Chromium over CDP
//! for bounded workflows (`mint browser run`) and CLI shell execution.

mod cdp;
mod evidence;
pub mod headless;
mod input;
mod interact;
pub(crate) mod lifecycle;
mod navigate;
mod overlay;
pub mod runtime;
mod session;
pub mod workflow;

pub use session::{
    BrowserActionResult, BrowserObservation, BrowserSession, capture_metadata, execute_action,
    run_session, with_tab,
};
pub(crate) use session::{finish_evidence as session_finish_evidence, selected_tab};

use serde::Serialize;

pub use input::{key_press, mouse_click, mouse_move, type_text_native};
pub use interact::{click, get_element_coordinates, type_text};
pub use navigate::{list_tabs, navigate, read_page_text, screenshot};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BrowserTab {
    pub id: String,
    pub title: String,
    pub url: String,
}

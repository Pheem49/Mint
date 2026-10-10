//! Native Desktop composition. Platform commands and state live with their owners.
mod app;
mod background;
mod commands;
mod native;

pub use app::lifecycle::run;
pub use commands::approvals::ApprovalsState;
pub use commands::settings::DetectedTools;
pub use commands::voice::{GeminiLiveState, MicRecordingState};

//! Shared color tokens for Mint's terminal interfaces.
//!
//! Keep terminal colors here so the TUI and ANSI output use the same small
//! palette instead of growing separate shades at each call site.

use ratatui::style::Color;

pub const ACCENT: Color = Color::Rgb(105, 230, 166); // #69E6A6
pub const BLUE: Color = Color::Rgb(96, 165, 250); // #60A5FA
pub const TEXT: Color = Color::White;
pub const MUTED: Color = Color::DarkGray;
pub const SUBTLE: Color = Color::Gray;
pub const WATERMARK_MARK: Color = Color::Rgb(105, 165, 138);
pub const WATERMARK_RIM: Color = Color::Rgb(77, 111, 109);
pub const WATERMARK_BAND: Color = Color::Rgb(54, 79, 80);
pub const WATERMARK_ORBIT: Color = Color::Rgb(78, 128, 112);
pub const WARNING: Color = Color::Yellow;
pub const SELECTION_TEXT: Color = Color::Black;
pub const SELECTION_BACKGROUND: Color = Color::Gray;
pub const PANEL_BACKGROUND: Color = Color::Rgb(38, 48, 45); // #26302D
/// Endpoints for the animated white glow used by the TUI thinking status.
pub const THINKING_GLOW_DIM: (u8, u8, u8) = (100, 100, 110);
pub const THINKING_GLOW_BRIGHT: (u8, u8, u8) = (255, 255, 255);

pub const ANSI_RESET: &str = "\x1b[0m";
pub const ANSI_BOLD: &str = "\x1b[1m";
pub const ANSI_ACCENT: &str = "\x1b[38;2;105;230;166m";
pub const ANSI_ACCENT_BOLD: &str = "\x1b[1;38;2;105;230;166m";
pub const ANSI_BLUE: &str = "\x1b[38;2;96;165;250m";
pub const ANSI_BLUE_BOLD: &str = "\x1b[1;38;2;96;165;250m";
pub const ANSI_TEXT: &str = "\x1b[97m";
pub const ANSI_BRIGHT_TEXT: &str = "\x1b[1;97m";
pub const ANSI_MUTED: &str = "\x1b[90m";
pub const ANSI_WARNING: &str = "\x1b[33m";
pub const ANSI_WARNING_BOLD: &str = "\x1b[1;33m";
pub const ANSI_ERROR: &str = "\x1b[31m";
pub const ANSI_ERROR_BOLD: &str = "\x1b[1;31m";
pub const ANSI_ADD: &str = "\x1b[48;2;38;48;45m\x1b[38;2;105;230;166m";
pub const ANSI_DELETE: &str = "\x1b[48;2;61;23;23m\x1b[31m";

/// Syntax highlighting keeps its own semantic palette for code tokens.
pub const SYNTAX_THEME: &str = "base16-ocean.dark";

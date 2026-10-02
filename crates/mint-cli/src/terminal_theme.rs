//! Shared color tokens for Mint's terminal interfaces.
//!
//! Keep terminal colors here so the TUI and ANSI output use the same small
//! palette instead of growing separate shades at each call site.

use ratatui::{buffer::Buffer, style::Color};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TuiTheme {
    Auto,
    Dark,
    Light,
}

impl Default for TuiTheme {
    fn default() -> Self {
        Self::Auto
    }
}

impl TuiTheme {
    pub fn from_config(value: &str) -> Self {
        match value {
            "system" | "auto" => Self::Auto,
            "light" => Self::Light,
            _ => Self::Dark,
        }
    }

    pub fn from_index(index: usize) -> Self {
        match index {
            0 => Self::Auto,
            2 => Self::Light,
            _ => Self::Dark,
        }
    }

    pub fn index(self) -> usize {
        match self {
            Self::Auto => 0,
            Self::Dark => 1,
            Self::Light => 2,
        }
    }

    pub fn config_value(self) -> &'static str {
        match self {
            Self::Auto => "system",
            Self::Dark => "dark",
            Self::Light => "light",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Auto => "Auto (match terminal)",
            Self::Dark => "Dark mode",
            Self::Light => "Light mode",
        }
    }
}

pub struct PreviewColors {
    pub background: Color,
    pub text: Color,
    pub muted: Color,
    pub keyword: Color,
    pub added: Color,
    pub removed: Color,
    pub added_background: Color,
    pub removed_background: Color,
}

pub fn preview_colors(theme: TuiTheme) -> PreviewColors {
    match theme {
        TuiTheme::Auto => PreviewColors {
            background: Color::Reset,
            text: Color::Reset,
            muted: MUTED,
            keyword: BLUE,
            added: ACCENT,
            removed: Color::Red,
            added_background: Color::Rgb(24, 55, 37),
            removed_background: Color::Rgb(57, 30, 30),
        },
        TuiTheme::Light => PreviewColors {
            background: Color::Rgb(244, 247, 244),
            text: Color::Rgb(35, 47, 42),
            muted: Color::Rgb(105, 116, 109),
            keyword: Color::Rgb(24, 99, 159),
            added: Color::Rgb(21, 111, 62),
            removed: Color::Rgb(171, 55, 55),
            added_background: Color::Rgb(221, 239, 225),
            removed_background: Color::Rgb(248, 229, 229),
        },
        TuiTheme::Dark => PreviewColors {
            background: Color::Rgb(29, 34, 33),
            text: Color::Rgb(226, 234, 228),
            muted: Color::Rgb(134, 148, 139),
            keyword: Color::Rgb(126, 176, 245),
            added: Color::Rgb(105, 230, 166),
            removed: Color::Rgb(238, 126, 126),
            added_background: Color::Rgb(24, 55, 37),
            removed_background: Color::Rgb(57, 30, 30),
        },
    }
}

pub fn apply_tui_theme(buffer: &mut Buffer, theme: TuiTheme) {
    for cell in &mut buffer.content {
        match theme {
            TuiTheme::Auto => {
                if cell.fg == Color::White {
                    cell.fg = Color::Reset;
                }
                if cell.bg == PANEL_BACKGROUND {
                    cell.bg = Color::Reset;
                }
            }
            TuiTheme::Dark => {
                if cell.fg == Color::Reset {
                    cell.fg = Color::Rgb(226, 234, 228);
                }
                if cell.bg == Color::Reset {
                    cell.bg = Color::Rgb(27, 30, 29);
                }
            }
            TuiTheme::Light => {
                cell.fg = match cell.fg {
                    Color::Reset | Color::White | Color::Black => Color::Rgb(35, 47, 42),
                    Color::DarkGray => Color::Rgb(104, 115, 108),
                    Color::Gray => Color::Rgb(79, 92, 84),
                    ACCENT | WATERMARK_MARK => Color::Rgb(20, 119, 77),
                    BLUE => Color::Rgb(31, 94, 165),
                    WATERMARK_RIM => Color::Rgb(86, 145, 122),
                    WATERMARK_BAND => Color::Rgb(171, 200, 182),
                    WATERMARK_ORBIT => Color::Rgb(112, 161, 132),
                    Color::Yellow => Color::Rgb(139, 93, 12),
                    other => other,
                };
                cell.bg = match cell.bg {
                    Color::Reset => Color::Rgb(247, 249, 246),
                    PANEL_BACKGROUND => Color::Rgb(228, 238, 230),
                    Color::Gray => Color::Rgb(185, 215, 196),
                    other => other,
                };
            }
        }
    }
}

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
pub const ANSI_BLUE_BOLD_UNDERLINE: &str = "\x1b[1m\x1b[4m\x1b[38;2;96;165;250m";
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

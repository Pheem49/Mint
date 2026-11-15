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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ThemeTokens {
    pub background: Color,
    pub panel_background: Color,
    pub user_message_background: Color,
    pub text: Color,
    pub muted: Color,
    pub subtle: Color,
    pub accent: Color,
    pub blue: Color,
    pub warning: Color,
    pub error: Color,
    pub added: Color,
    pub removed: Color,
    pub added_background: Color,
    pub removed_background: Color,
    pub watermark_mark: Color,
    pub watermark_rim: Color,
    pub watermark_band: Color,
    pub watermark_orbit: Color,
    pub selection_background: Color,
}

pub const fn theme_tokens(theme: TuiTheme) -> ThemeTokens {
    match theme {
        TuiTheme::Auto => ThemeTokens {
            background: Color::Reset,
            panel_background: Color::Reset,
            user_message_background: USER_MESSAGE_BACKGROUND,
            text: Color::Reset,
            muted: MUTED,
            subtle: SUBTLE,
            accent: ACCENT,
            blue: BLUE,
            warning: WARNING,
            error: Color::Red,
            added: ACCENT,
            removed: Color::Red,
            added_background: Color::Rgb(24, 55, 37),
            removed_background: Color::Rgb(57, 30, 30),
            watermark_mark: WATERMARK_MARK,
            watermark_rim: WATERMARK_RIM,
            watermark_band: WATERMARK_BAND,
            watermark_orbit: WATERMARK_ORBIT,
            selection_background: SELECTION_BACKGROUND,
        },
        TuiTheme::Dark => ThemeTokens {
            background: Color::Rgb(27, 30, 29),
            panel_background: PANEL_BACKGROUND,
            user_message_background: USER_MESSAGE_BACKGROUND,
            text: Color::Rgb(226, 234, 228),
            muted: Color::Rgb(134, 148, 139),
            subtle: SUBTLE,
            accent: ACCENT,
            blue: Color::Rgb(126, 176, 245),
            warning: WARNING,
            error: Color::Rgb(238, 126, 126),
            added: ACCENT,
            removed: Color::Rgb(238, 126, 126),
            added_background: Color::Rgb(24, 55, 37),
            removed_background: Color::Rgb(57, 30, 30),
            watermark_mark: WATERMARK_MARK,
            watermark_rim: WATERMARK_RIM,
            watermark_band: WATERMARK_BAND,
            watermark_orbit: WATERMARK_ORBIT,
            selection_background: SELECTION_BACKGROUND,
        },
        TuiTheme::Light => ThemeTokens {
            background: Color::Rgb(247, 249, 246),
            panel_background: Color::Rgb(228, 238, 230),
            user_message_background: Color::Rgb(225, 225, 225),
            text: Color::Rgb(35, 47, 42),
            muted: Color::Rgb(104, 115, 108),
            subtle: Color::Rgb(79, 92, 84),
            accent: Color::Rgb(20, 119, 77),
            blue: Color::Rgb(31, 94, 165),
            warning: Color::Rgb(139, 93, 12),
            error: Color::Rgb(171, 55, 55),
            added: Color::Rgb(21, 111, 62),
            removed: Color::Rgb(171, 55, 55),
            added_background: Color::Rgb(221, 239, 225),
            removed_background: Color::Rgb(248, 229, 229),
            watermark_mark: Color::Rgb(20, 119, 77),
            watermark_rim: Color::Rgb(86, 145, 122),
            watermark_band: Color::Rgb(171, 200, 182),
            watermark_orbit: Color::Rgb(112, 161, 132),
            selection_background: Color::Rgb(185, 215, 196),
        },
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
    let tokens = theme_tokens(theme);
    PreviewColors {
        background: match theme {
            TuiTheme::Dark => Color::Rgb(29, 34, 33),
            TuiTheme::Light => Color::Rgb(244, 247, 244),
            TuiTheme::Auto => tokens.background,
        },
        text: tokens.text,
        muted: match theme {
            TuiTheme::Light => Color::Rgb(105, 116, 109),
            _ => tokens.muted,
        },
        keyword: match theme {
            TuiTheme::Light => Color::Rgb(24, 99, 159),
            _ => tokens.blue,
        },
        added: tokens.added,
        removed: tokens.removed,
        added_background: tokens.added_background,
        removed_background: tokens.removed_background,
    }
}

pub fn apply_tui_theme(buffer: &mut Buffer, theme: TuiTheme) {
    let tokens = theme_tokens(theme);
    for cell in &mut buffer.content {
        match theme {
            TuiTheme::Auto => {
                if cell.fg == Color::White && cell.bg != USER_MESSAGE_BACKGROUND {
                    cell.fg = tokens.text;
                }
                if cell.bg == PANEL_BACKGROUND {
                    cell.bg = tokens.panel_background;
                }
            }
            TuiTheme::Dark => {
                if cell.fg == Color::Reset {
                    cell.fg = tokens.text;
                }
                if cell.bg == Color::Reset {
                    cell.bg = tokens.background;
                }
            }
            TuiTheme::Light => {
                cell.fg = match cell.fg {
                    Color::Reset | Color::White | Color::Black => tokens.text,
                    Color::DarkGray => tokens.muted,
                    Color::Gray => tokens.subtle,
                    ACCENT | WATERMARK_MARK => tokens.accent,
                    BLUE => tokens.blue,
                    CARD_BORDER => Color::Rgb(175, 180, 188),
                    WATERMARK_RIM => tokens.watermark_rim,
                    WATERMARK_BAND => tokens.watermark_band,
                    WATERMARK_ORBIT => tokens.watermark_orbit,
                    Color::Yellow => tokens.warning,
                    other => other,
                };
                cell.bg = match cell.bg {
                    Color::Reset => tokens.background,
                    PANEL_BACKGROUND => tokens.panel_background,
                    USER_MESSAGE_BACKGROUND => tokens.user_message_background,
                    Color::Gray => tokens.selection_background,
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
pub const USER_MESSAGE_BACKGROUND: Color = Color::Rgb(64, 64, 64); // #404040
/// Gray card surface and border tokens for framed transcript cards (e.g. Release Notes).
pub const CARD_BACKGROUND: Color = USER_MESSAGE_BACKGROUND; // Color::Rgb(64, 64, 64) - matches user message bubble
pub const CARD_BORDER: Color = Color::Rgb(115, 120, 130); // #737882 - clear neutral gray border
/// Endpoints for the animated white glow used by the TUI thinking status.
pub const THINKING_GLOW_DIM: (u8, u8, u8) = (100, 100, 110);
pub const THINKING_GLOW_BRIGHT: (u8, u8, u8) = (255, 255, 255);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn theme_tokens_and_preview_colors_remain_aligned() {
        for theme in [TuiTheme::Auto, TuiTheme::Dark, TuiTheme::Light] {
            let tokens = theme_tokens(theme);
            let preview = preview_colors(theme);
            assert_eq!(preview.text, tokens.text);
            assert_eq!(preview.added, tokens.added);
            assert_eq!(preview.removed, tokens.removed);
            assert_eq!(preview.added_background, tokens.added_background);
            assert_eq!(preview.removed_background, tokens.removed_background);
        }
    }

    #[test]
    fn card_tokens_preserve_gray_background_in_auto_and_adapt_in_light() {
        use ratatui::layout::Rect;

        // In Auto theme, card background must not get cleared to Reset
        let mut buf_auto = Buffer::empty(Rect::new(0, 0, 2, 2));
        buf_auto[(0, 0)].set_bg(CARD_BACKGROUND).set_fg(CARD_BORDER);
        apply_tui_theme(&mut buf_auto, TuiTheme::Auto);
        assert_eq!(buf_auto[(0, 0)].bg, CARD_BACKGROUND);
        assert_eq!(buf_auto[(0, 0)].fg, CARD_BORDER);

        // In Light theme, card background and border adapt to light surface
        let mut buf_light = Buffer::empty(Rect::new(0, 0, 2, 2));
        buf_light[(0, 0)]
            .set_bg(CARD_BACKGROUND)
            .set_fg(CARD_BORDER);
        apply_tui_theme(&mut buf_light, TuiTheme::Light);
        assert_eq!(buf_light[(0, 0)].bg, Color::Rgb(225, 225, 225));
        assert_eq!(buf_light[(0, 0)].fg, Color::Rgb(175, 180, 188));
    }
}

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

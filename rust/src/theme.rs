//! Tokyo Night ("Night" variant) color palette used throughout the TUI.

use ratatui::style::Color;

/// Pane background.
pub const BG: Color = Color::Rgb(0x1a, 0x1b, 0x26);
/// Background for the currently selected result row.
pub const BG_HIGHLIGHT: Color = Color::Rgb(0x29, 0x2e, 0x42);
/// Background for the matched line in the preview pane.
pub const MATCH_LINE_BG: Color = Color::Rgb(0x2d, 0x3f, 0x76);
/// Primary text color.
pub const FG: Color = Color::Rgb(0xc0, 0xca, 0xf5);
/// Muted text color for secondary/status text.
pub const MUTED: Color = Color::Rgb(0x56, 0x5f, 0x89);
/// Accent color for borders and titles.
pub const ACCENT: Color = Color::Rgb(0x7a, 0xa2, 0xf7);

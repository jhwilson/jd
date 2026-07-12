//! The TUI palette. Swapping to RGB Okabe–Ito later is a one-file change; if
//! selection is too subtle, adding `.bg(Color::DarkGray)` to `SELECTED` is the
//! one-line escape hatch.

use ratatui::style::{Color, Modifier, Style};

pub const LABEL: Style = Style::new().add_modifier(Modifier::DIM);
pub const ACCENT: Style = Style::new().fg(Color::Blue);
pub const MATCH: Style = Style::new().fg(Color::Cyan).add_modifier(Modifier::BOLD);
pub const SELECTED: Style = Style::new().fg(Color::Blue).add_modifier(Modifier::BOLD);
pub const SELECT_MARK: &str = "▌ ";
pub const HINT: Style = Style::new().add_modifier(Modifier::DIM);
pub const MUTED: Style = Style::new().add_modifier(Modifier::DIM);
pub const OK: Style = Style::new().fg(Color::Blue);
pub const WARN: Style = Style::new().fg(Color::Yellow);
pub const ERR: Style = Style::new().fg(Color::Red).add_modifier(Modifier::BOLD);
pub const RULE: Style = Style::new().add_modifier(Modifier::DIM);

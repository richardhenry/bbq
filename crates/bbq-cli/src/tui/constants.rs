use ratatui::style::Color;

pub(crate) const SELECTED_TEXT: Color = Color::Rgb(20, 20, 20);
pub(crate) const SELECTED_SECONDARY: Color = Color::Rgb(90, 90, 90);

pub(crate) const STATUS_MIN_MS: u64 = 2000;
pub(crate) const STATUS_PER_CHAR_MS: u64 = 30;
pub(crate) const STATUS_MAX_MS: u64 = 8000;

pub(crate) const SPINNER_INTERVAL_MS: u128 = 120;
pub(crate) const SPINNER_FRAMES: [&str; 10] = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];

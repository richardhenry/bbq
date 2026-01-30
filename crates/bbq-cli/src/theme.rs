use ratatui::style::Color;

#[derive(Debug, Clone, Copy)]
pub(crate) struct Theme {
    pub(crate) name: &'static str,
    pub(crate) rgb: (u8, u8, u8),
}

impl Theme {
    pub(crate) const fn new(name: &'static str, rgb: (u8, u8, u8)) -> Self {
        Self { name, rgb }
    }

    pub(crate) fn color(&self) -> Color {
        let (r, g, b) = self.rgb;
        Color::Rgb(r, g, b)
    }
}

pub(crate) const THEMES: [Theme; 13] = [
    Theme::new("green", (0, 255, 0)),
    Theme::new("red", (255, 0, 0)),
    Theme::new("blue", (0, 0, 255)),
    Theme::new("skyblue", (135, 206, 235)),
    Theme::new("magenta", (255, 0, 255)),
    Theme::new("yellow", (255, 255, 0)),
    Theme::new("gold", (255, 215, 0)),
    Theme::new("silver", (192, 192, 192)),
    Theme::new("white", (255, 255, 255)),
    Theme::new("lime", (191, 255, 0)),
    Theme::new("orange", (255, 165, 0)),
    Theme::new("violet", (148, 0, 211)),
    Theme::new("pink", (255, 105, 180)),
];

pub(crate) fn default_theme_index() -> usize {
    THEMES
        .iter()
        .position(|theme| theme.name == "orange")
        .unwrap_or(0)
}

pub(crate) fn theme_index_by_name(name: &str) -> Option<usize> {
    THEMES
        .iter()
        .position(|theme| theme.name.eq_ignore_ascii_case(name))
}

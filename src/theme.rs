//! Color themes: the plain GTK light and dark styles, plus named palettes
//! drawn as CSS on top of whichever of the two matches their brightness.

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Theme {
    Light,
    Dark,
    Nord,
    SolarizedDark,
    SolarizedLight,
    Dracula,
    GruvboxDark,
    GruvboxLight,
    CatppuccinMocha,
    CatppuccinLatte,
    TokyoNight,
    OneDark,
}

/// The named palettes, in menu order.
pub const PALETTES: &[Theme] = &[
    Theme::Nord,
    Theme::SolarizedDark,
    Theme::SolarizedLight,
    Theme::Dracula,
    Theme::GruvboxDark,
    Theme::GruvboxLight,
    Theme::CatppuccinMocha,
    Theme::CatppuccinLatte,
    Theme::TokyoNight,
    Theme::OneDark,
];

struct Palette {
    dark: bool,
    bg: &'static str,
    /// Keys and the entry.
    surface: &'static str,
    fg: &'static str,
    /// `=` and selection.
    accent: &'static str,
    /// Clear and error messages.
    danger: &'static str,
}

impl Theme {
    pub fn key(self) -> &'static str {
        match self {
            Theme::Light => "light",
            Theme::Dark => "dark",
            Theme::Nord => "nord",
            Theme::SolarizedDark => "solarized-dark",
            Theme::SolarizedLight => "solarized-light",
            Theme::Dracula => "dracula",
            Theme::GruvboxDark => "gruvbox-dark",
            Theme::GruvboxLight => "gruvbox-light",
            Theme::CatppuccinMocha => "catppuccin-mocha",
            Theme::CatppuccinLatte => "catppuccin-latte",
            Theme::TokyoNight => "tokyo-night",
            Theme::OneDark => "one-dark",
        }
    }

    pub fn from_key(key: &str) -> Option<Self> {
        [Theme::Light, Theme::Dark]
            .iter()
            .chain(PALETTES)
            .copied()
            .find(|t| t.key() == key)
    }

    /// Display name of a named palette; `Light` and `Dark` are translated instead.
    pub fn name(self) -> &'static str {
        match self {
            Theme::Light => "Light",
            Theme::Dark => "Dark",
            Theme::Nord => "Nord",
            Theme::SolarizedDark => "Solarized Dark",
            Theme::SolarizedLight => "Solarized Light",
            Theme::Dracula => "Dracula",
            Theme::GruvboxDark => "Gruvbox Dark",
            Theme::GruvboxLight => "Gruvbox Light",
            Theme::CatppuccinMocha => "Catppuccin Mocha",
            Theme::CatppuccinLatte => "Catppuccin Latte",
            Theme::TokyoNight => "Tokyo Night",
            Theme::OneDark => "One Dark",
        }
    }

    fn palette(self) -> Option<Palette> {
        let p = |dark, bg, surface, fg, accent, danger| {
            Some(Palette { dark, bg, surface, fg, accent, danger })
        };
        match self {
            Theme::Light | Theme::Dark => None,
            Theme::Nord => p(true, "#2e3440", "#3b4252", "#eceff4", "#88c0d0", "#bf616a"),
            Theme::SolarizedDark => p(true, "#002b36", "#073642", "#93a1a1", "#268bd2", "#dc322f"),
            Theme::SolarizedLight => p(false, "#fdf6e3", "#eee8d5", "#586e75", "#268bd2", "#dc322f"),
            Theme::Dracula => p(true, "#282a36", "#44475a", "#f8f8f2", "#bd93f9", "#ff5555"),
            Theme::GruvboxDark => p(true, "#282828", "#3c3836", "#ebdbb2", "#fabd2f", "#fb4934"),
            Theme::GruvboxLight => p(false, "#fbf1c7", "#ebdbb2", "#3c3836", "#076678", "#9d0006"),
            Theme::CatppuccinMocha => p(true, "#1e1e2e", "#313244", "#cdd6f4", "#cba6f7", "#f38ba8"),
            Theme::CatppuccinLatte => p(false, "#eff1f5", "#ccd0da", "#4c4f69", "#8839ef", "#d20f39"),
            Theme::TokyoNight => p(true, "#1a1b26", "#24283b", "#c0caf5", "#7aa2f7", "#f7768e"),
            Theme::OneDark => p(true, "#282c34", "#3e4451", "#abb2bf", "#61afef", "#e06c75"),
        }
    }

    /// Switches the GTK base style and loads the palette CSS into `provider`.
    pub fn apply(self, provider: &gtk::CssProvider) {
        let palette = self.palette();
        let dark = palette.as_ref().map_or(self == Theme::Dark, |p| p.dark);
        if let Some(settings) = gtk::Settings::default() {
            settings.set_gtk_application_prefer_dark_theme(dark);
        }
        provider.load_from_string(&palette.map(|p| p.css()).unwrap_or_default());
    }
}

impl Palette {
    fn css(&self) -> String {
        let Palette { bg, surface, fg, accent, danger, .. } = self;
        format!(
            "
window.background, menubar, popover.menu > contents {{
    background-color: {bg}; color: {fg};
}}
menubar > item:hover, menubar > item:selected,
popover.menu modelbutton:hover, popover.menu modelbutton:selected {{
    background-color: {surface}; color: {fg};
}}
entry {{
    background-color: {surface}; color: {fg};
    border-color: mix({surface}, {fg}, 0.2); box-shadow: none;
}}
entry:focus-within {{ outline-color: alpha({accent}, 0.6); }}
entry > text > selection {{ background-color: alpha({accent}, 0.4); color: {fg}; }}
grid.keys button {{
    background-image: none; background-color: {surface}; color: {fg};
    border-color: transparent; box-shadow: none;
}}
grid.keys button:hover {{ background-color: mix({surface}, {fg}, 0.1); }}
grid.keys button:active {{ background-color: mix({surface}, {fg}, 0.2); }}
grid.keys button.suggested-action {{ background-color: {accent}; color: {bg}; }}
grid.keys button.suggested-action:hover {{ background-color: mix({accent}, {fg}, 0.15); }}
grid.keys button.destructive-action {{ background-color: {danger}; color: {bg}; }}
grid.keys button.destructive-action:hover {{ background-color: mix({danger}, {fg}, 0.15); }}
button.result {{ color: {fg}; }}
button.result:hover {{ background-color: alpha({fg}, 0.08); }}
button.result.error label {{ color: {danger}; }}
"
        )
    }
}

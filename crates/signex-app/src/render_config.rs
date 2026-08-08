//! Render-configuration *types* for signex-app — the enums the appearance
//! preferences are expressed in, plus the two shared render helpers.
//!
//! #630 — this module used to also own a process-wide
//! `OnceLock<RwLock<CanvasTextConfig>>` that duplicated nine `UiState`
//! fields, was kept in sync by hand across 26 call sites, and leaked a
//! fresh `Box::leak`ed font name on every font change. It is gone: the
//! values live in `UiState` alone and reach the canvases as ordinary
//! fields (`SchematicCanvas::grid_style`, `SymbolCanvas::grid_style` via
//! `PanelContext::symbol_grid_style`). Nothing here holds state — do not
//! reintroduce a global; a `draw` path that reads one is not a function
//! of the app state.

pub const IOSEVKA: iced::Font = iced::Font::with_name("Iosevka");

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PowerPortStyle {
    Standard,
    #[default]
    Altium,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LabelStyle {
    #[default]
    Standard,
    Altium,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum MultisheetStyle {
    #[default]
    Standard,
    Altium,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum GridStyle {
    #[default]
    Dots,
    Lines,
    SmallCrosses,
}

impl GridStyle {
    pub const ALL: &'static [GridStyle] =
        &[GridStyle::Dots, GridStyle::Lines, GridStyle::SmallCrosses];
}

impl std::fmt::Display for PowerPortStyle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PowerPortStyle::Standard => write!(f, "Standard"),
            PowerPortStyle::Altium => write!(f, "Altium"),
        }
    }
}

impl std::fmt::Display for LabelStyle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LabelStyle::Standard => write!(f, "Standard"),
            LabelStyle::Altium => write!(f, "Altium"),
        }
    }
}

impl std::fmt::Display for MultisheetStyle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MultisheetStyle::Standard => write!(f, "Standard"),
            MultisheetStyle::Altium => write!(f, "Altium"),
        }
    }
}

impl std::fmt::Display for GridStyle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            GridStyle::Dots => write!(f, "Dots"),
            GridStyle::Lines => write!(f, "Lines"),
            GridStyle::SmallCrosses => write!(f, "Small crosses"),
        }
    }
}

/// How a click selects/drags a pin in the symbol editor.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PinSelectionMode {
    /// Altium parity — only the pin body/tip is grabbable.
    #[default]
    PinOnly,
    /// The pin is also grabbable by its name or number label, and a
    /// selected pin's labels glow with it.
    TextAndPin,
}

impl PinSelectionMode {
    pub const ALL: [PinSelectionMode; 2] =
        [PinSelectionMode::PinOnly, PinSelectionMode::TextAndPin];
    /// True when name/number labels are grabbable + glow.
    pub fn allows_label_grab(self) -> bool {
        matches!(self, PinSelectionMode::TextAndPin)
    }

    /// Stable token used to persist this mode to `prefs.json`.
    pub fn pref_token(self) -> &'static str {
        match self {
            PinSelectionMode::PinOnly => "pin_only",
            PinSelectionMode::TextAndPin => "text_and_pin",
        }
    }

    /// Parse a persisted token back into a mode — unknown/legacy values
    /// fall back to the `PinOnly` default.
    pub fn from_pref_token(s: &str) -> Self {
        match s {
            "text_and_pin" => PinSelectionMode::TextAndPin,
            _ => PinSelectionMode::PinOnly,
        }
    }
}

impl std::fmt::Display for PinSelectionMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            PinSelectionMode::PinOnly => "Pin only",
            PinSelectionMode::TextAndPin => "Text and pin",
        })
    }
}

pub fn to_iced(c: &signex_types::theme::Color) -> iced::Color {
    iced::Color::from_rgba8(c.r, c.g, c.b, c.a as f32 / 255.0)
}

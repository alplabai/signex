//! Render-configuration *types* for signex-app — the enums the appearance
//! preferences are expressed in, plus the two shared render helpers.
//!
//! #630 — this module used to also own a process-wide
//! `OnceLock<RwLock<CanvasTextConfig>>` that duplicated nine `UiState`
//! fields, was kept in sync by hand across 26 call sites, and leaked a
//! fresh `Box::leak`ed font name on every font change. It is gone: the
//! values live in `UiState` alone and reach the canvases as ordinary
//! fields (`CanvasViewPrefs::grid_style`, and `SymbolCanvas::grid_style` via
//! `PanelContext::symbol_grid_style`). Nothing here holds state — do not
//! reintroduce a global; a `draw` path that reads one is not a function
//! of the app state.

/// Family name of the canvas font, as registered with iced by `main.rs`.
///
/// One string, shared by the CPU replay's [`IOSEVKA`] and the GPU text
/// pipeline — the two shaping against different families is what made
/// schematic text render in the wrong typeface on the GPU path.
pub const CANVAS_FONT_FAMILY: &str = "Iosevka";

pub const IOSEVKA: iced::Font = iced::Font::new(CANVAS_FONT_FAMILY);

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

impl std::fmt::Display for GridStyle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            GridStyle::Dots => write!(f, "Dots"),
            GridStyle::Lines => write!(f, "Lines"),
            GridStyle::SmallCrosses => write!(f, "Small crosses"),
        }
    }
}

pub use crate::app::core::pin_selection_mode::PinSelectionMode;

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

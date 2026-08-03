//! The theme-derived colour set the Properties panel paints with.
//!
//! Every Properties-panel renderer needs some subset of the same eight
//! colours, and before this module each one took them as loose
//! positional `Color` parameters. That had two costs. Deriving them was
//! copy-pasted into five call sites (`panels::properties::view_properties`,
//! `properties_parameters::general::view_properties_general`,
//! `element_properties::selected`, `element_properties::child_sheet`,
//! `footprint_editor_properties`), so a token change had to be applied
//! five times to stay consistent. And every colour has the same type, so
//! a transposed pair — `input_bg` where `input_bdr` was meant — compiled
//! silently and shipped the wrong chrome. Naming the fields makes that a
//! compile error instead.

use iced::Color;
use signex_types::theme::ThemeTokens;
use signex_widgets::theme_ext;

/// Hover-tint multiplier for the Custom Selection Filters preset chips.
/// Brightens the accent colour without changing its alpha.
const TAG_HOVER_BRIGHTEN: f32 = 1.3;

/// Every theme colour the Properties panel and its sub-forms render with.
///
/// Built once per frame by `panels::properties::view_properties` and
/// `properties_parameters::general::view_properties_general`, then passed
/// down by value — all eight fields are `Copy`, so each hop costs a move
/// rather than a fresh borrow of the tokens.
#[derive(Debug, Clone, Copy)]
pub struct PanelPalette {
    /// Secondary label text (field names, hints, disabled cells).
    pub muted: Color,
    /// Primary text (values, section headers, button labels).
    pub primary: Color,
    /// Separator + section-header rule colour.
    pub border: Color,
    /// Text-input / segmented-button fill (deep blue selection tint).
    pub input_bg: Color,
    /// Text-input border + active segment fill.
    pub input_bdr: Color,
    /// Raw accent, used for the multi-select tag and preset chips.
    pub accent: Color,
    /// Hovered preset chip — the accent brightened by
    /// [`TAG_HOVER_BRIGHTEN`].
    pub tag_hover: Color,
    /// Hovered segmented-button cell.
    pub seg_hover: Color,
}

impl PanelPalette {
    /// Derive the panel palette from the active theme tokens.
    ///
    /// Reproduces, verbatim, the per-call-site derivations this struct
    /// replaced — same helpers, same token fields, same brighten maths.
    pub fn from_tokens(tokens: &ThemeTokens) -> Self {
        let accent = crate::styles::ti(tokens.accent);
        Self {
            muted: theme_ext::text_secondary(tokens),
            primary: theme_ext::text_primary(tokens),
            border: theme_ext::border_color(tokens),
            input_bg: crate::styles::ti(tokens.selection),
            input_bdr: accent,
            accent,
            tag_hover: Color {
                r: (accent.r * TAG_HOVER_BRIGHTEN).min(1.0),
                g: (accent.g * TAG_HOVER_BRIGHTEN).min(1.0),
                b: (accent.b * TAG_HOVER_BRIGHTEN).min(1.0),
                ..accent
            },
            seg_hover: crate::styles::ti(tokens.hover),
        }
    }
}

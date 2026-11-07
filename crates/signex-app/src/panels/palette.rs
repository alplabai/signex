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

/// Ink and soft fill of one DOF chip state. `fill` is `None` on legacy
/// themes, which draw the state as coloured text.
#[derive(Debug, Clone, Copy)]
pub struct DofLook {
    pub ink: Color,
    pub fill: Option<Color>,
}

/// The DOF chip looks per solver state: under-constrained is `info`, fully
/// constrained `ok`, over-constrained `err`.
#[derive(Debug, Clone, Copy)]
pub struct DofLooks {
    pub under: DofLook,
    pub full: DofLook,
    pub over: DofLook,
}

impl DofLooks {
    pub fn for_tokens(tokens: &ThemeTokens) -> Self {
        use crate::styles::{PillTone, is_graphite, pill_colors};
        if is_graphite(tokens) {
            let look = |tone| {
                let (fill, ink) = pill_colors(tokens, tone);
                DofLook {
                    ink,
                    fill: Some(fill),
                }
            };
            return Self {
                under: look(PillTone::Info),
                full: look(PillTone::Ok),
                over: look(PillTone::Err),
            };
        }
        let plain = |ink| DofLook { ink, fill: None };
        Self {
            under: plain(Color::from_rgba(0.20, 0.40, 1.00, 1.00)),
            full: plain(Color::from_rgba(0.20, 0.85, 0.30, 1.00)),
            over: plain(Color::from_rgba(1.00, 0.20, 0.20, 1.00)),
        }
    }
}

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
    /// Surface the panel is drawn on; text placed directly on it picks
    /// its colour against this, not against the iced palette.
    pub panel_bg: Color,
    /// DOF chip looks of the sketch entity row.
    pub dof: DofLooks,
    /// Whether the panel is drawn in a Graphite theme.
    pub graphite: bool,
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
            panel_bg: crate::styles::ti(tokens.panel_bg),
            dof: DofLooks::for_tokens(tokens),
            graphite: crate::styles::is_graphite(tokens),
        }
    }

    /// `(label, value)` widths of a footprint Properties field row: the legacy
    /// 110 px label, or the Graphite 4:5 split every other form row uses.
    pub fn field_widths(&self) -> (iced::Length, iced::Length) {
        if self.graphite {
            (iced::Length::FillPortion(4), iced::Length::FillPortion(5))
        } else {
            (iced::Length::Fixed(110.0), iced::Length::Fill)
        }
    }
}

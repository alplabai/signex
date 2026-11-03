//! Footprint preview colours, all derived from the UI theme tokens so the
//! board reads on light and dark themes alike.

use iced::Color;
use signex_types::theme::ThemeTokens;
use signex_widgets::theme_ext;

use crate::styles::tokens;

/// Share of a layer colour kept for a fill under its outline.
const FILL_ALPHA: f32 = 0.18;

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct Palette {
    /// The board surface behind the footprint.
    pub backdrop: Color,
    pub pad: Color,
    /// Pad numbers, readable on the pad fill.
    pub pad_text: Color,
    pub silk_top: Color,
    pub silk_bottom: Color,
    pub courtyard: Color,
    /// Fab / body outline.
    pub fab: Color,
    /// Size and scale readouts.
    pub readout: Color,
}

impl Palette {
    pub fn from_tokens(tokens: &ThemeTokens) -> Self {
        let pad = theme_ext::accent(tokens);
        Self {
            backdrop: theme_ext::to_color(&tokens.paper),
            pad,
            pad_text: tokens::readable_on(pad),
            silk_top: theme_ext::text_primary(tokens),
            silk_bottom: theme_ext::text_secondary(tokens),
            courtyard: theme_ext::warning_color(tokens),
            fab: theme_ext::text_secondary(tokens),
            readout: theme_ext::text_secondary(tokens),
        }
    }
}

/// `colour` thinned to a translucent fill.
pub(super) fn fill_of(colour: Color) -> Color {
    Color {
        a: colour.a * FILL_ALPHA,
        ..colour
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use signex_types::theme::{ThemeId, theme_tokens};

    #[test]
    fn the_board_stands_apart_from_its_layers_on_every_theme() {
        for id in ThemeId::BUILTINS {
            let p = Palette::from_tokens(&theme_tokens(*id));
            for (name, layer) in [
                ("pad", p.pad),
                ("silk", p.silk_top),
                ("courtyard", p.courtyard),
                ("fab", p.fab),
            ] {
                assert_ne!(layer, p.backdrop, "{name} vanishes on {id:?}");
            }
        }
    }

    #[test]
    fn a_fill_is_the_layer_colour_made_translucent() {
        let c = Color::from_rgb(0.2, 0.4, 0.6);
        let f = fill_of(c);
        assert_eq!((f.r, f.g, f.b), (c.r, c.g, c.b));
        assert!(f.a < c.a);
    }
}

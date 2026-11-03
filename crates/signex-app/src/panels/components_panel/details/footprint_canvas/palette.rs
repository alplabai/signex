//! Footprint preview colours. Layers use the footprint editor's palette
//! (`FpLayer::color`) so one footprint looks the same in both places; the
//! board is a dark surface derived from the theme paper, since that palette
//! is tuned for a dark canvas (near-white silk). Keep the two in step by
//! sharing `FpLayer`, not by copying colours.

use iced::Color;
use signex_types::theme::ThemeTokens;
use signex_widgets::theme_ext;

use crate::library::editor::footprint::layers::FpLayer;
use crate::styles::tokens;

/// Share of a layer colour kept for a fill under its outline.
const FILL_ALPHA: f32 = 0.18;
/// Share of the theme paper colour kept in the dark board surface.
const BOARD_PAPER_SHARE: f32 = 0.12;

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
        let paper = theme_ext::to_color(&tokens.paper);
        let backdrop = Color {
            r: paper.r * BOARD_PAPER_SHARE,
            g: paper.g * BOARD_PAPER_SHARE,
            b: paper.b * BOARD_PAPER_SHARE,
            a: 1.0,
        };
        let pad = FpLayer::FCu.color();
        Self {
            backdrop,
            pad,
            pad_text: tokens::readable_on(pad),
            silk_top: FpLayer::FSilks.color(),
            silk_bottom: FpLayer::BSilks.color(),
            courtyard: FpLayer::EdgeCuts.color(),
            fab: FpLayer::FFab.color(),
            readout: tokens::readable_on(backdrop),
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
    /// Floor for a layer stroke against the board.
    const MIN_LAYER_CONTRAST: f32 = 3.0;

    use super::*;
    use signex_types::theme::{ThemeId, theme_tokens};

    #[test]
    fn layers_are_distinct_and_stand_out_from_the_board_on_every_theme() {
        for id in ThemeId::BUILTINS {
            let p = Palette::from_tokens(&theme_tokens(*id));
            let layers = [
                ("pad", p.pad),
                ("silk", p.silk_top),
                ("courtyard", p.courtyard),
                ("fab", p.fab),
            ];
            for (i, (name, layer)) in layers.iter().enumerate() {
                assert!(
                    tokens::contrast_ratio(*layer, p.backdrop) >= MIN_LAYER_CONTRAST,
                    "{name} is faint on {id:?}"
                );
                for (other, colour) in &layers[i + 1..] {
                    assert_ne!(layer, colour, "{name} and {other} match on {id:?}");
                }
            }
        }
    }

    #[test]
    fn readout_text_is_readable_on_the_board_on_every_theme() {
        for id in ThemeId::BUILTINS {
            let p = Palette::from_tokens(&theme_tokens(*id));
            assert!(
                tokens::contrast_ratio(p.readout, p.backdrop) >= tokens::MIN_TEXT_CONTRAST,
                "readout unreadable on {id:?}"
            );
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

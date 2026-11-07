//! The Graphite colours the schematic canvas reads from the chrome tokens:
//! the sheet-frame line, the selection halo and the ERC marker inks.
//! `None` on every other theme, which keeps its old canvas look.

use iced::Color;
use signex_types::theme::ThemeTokens;

use crate::render_config::to_iced;
use crate::styles::{is_graphite, tokens::resolve};

/// Alpha of the selection halo's `accent_soft` fill, so the part under it
/// still reads.
pub const HALO_FILL_ALPHA: f32 = 0.5;

/// Chrome-token colours the Graphite canvas draws with.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GraphiteCanvasInk {
    /// Sheet frame and title-block lines.
    pub line_strong: Color,
    /// Selection halo outline.
    pub accent: Color,
    /// Selection halo fill (before [`HALO_FILL_ALPHA`]).
    pub accent_soft: Color,
    /// ERC warning marker.
    pub warn: Color,
    /// ERC error marker.
    pub err: Color,
}

impl GraphiteCanvasInk {
    /// The canvas inks of `tokens`, or `None` when they are not Graphite.
    #[must_use]
    pub fn from_tokens(tokens: &ThemeTokens) -> Option<Self> {
        if !is_graphite(tokens) {
            return None;
        }
        let r = resolve(tokens);
        Some(Self {
            line_strong: r.line_strong,
            accent: to_iced(&tokens.accent),
            accent_soft: r.accent_soft,
            warn: to_iced(&tokens.warning),
            err: to_iced(&tokens.error),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use signex_types::theme::{ThemeId, theme_tokens};

    #[test]
    fn every_theme_has_canvas_inks_and_legacy_tokens_none() {
        for &id in ThemeId::BUILTINS {
            let ink = GraphiteCanvasInk::from_tokens(&theme_tokens(id));
            assert!(ink.is_some(), "{id:?}");
        }
        let legacy = crate::styles::legacy_tokens_fixture();
        assert!(GraphiteCanvasInk::from_tokens(&legacy).is_none());
    }

    #[test]
    fn the_inks_are_the_token_colours() {
        let tokens = theme_tokens(ThemeId::GraphiteLight);
        let ink = GraphiteCanvasInk::from_tokens(&tokens).expect("graphite");
        assert_eq!(
            ink.line_strong,
            to_iced(&signex_types::theme::Color::new(0xDC, 0xD9, 0xD1, 255))
        );
        assert_eq!(ink.err, to_iced(&tokens.error));
        assert_eq!(ink.warn, to_iced(&tokens.warning));
    }
}

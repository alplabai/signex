//! The Graphite colours the schematic canvas reads from the chrome tokens:
//! the sheet-frame line, the selection halo and the ERC marker inks.

use iced::Color;
use signex_types::theme::ThemeTokens;

use crate::render_config::to_iced;
use crate::styles::tokens::resolve;

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
    /// The canvas inks of `tokens`.
    #[must_use]
    pub fn from_tokens(tokens: &ThemeTokens) -> Self {
        let r = resolve(tokens);
        Self {
            line_strong: r.line_strong,
            accent: to_iced(&tokens.accent),
            accent_soft: r.accent_soft,
            warn: to_iced(&tokens.warning),
            err: to_iced(&tokens.error),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use signex_types::theme::{ThemeId, theme_tokens};

    #[test]
    fn the_inks_are_the_token_colours() {
        let tokens = theme_tokens(ThemeId::GraphiteLight);
        let ink = GraphiteCanvasInk::from_tokens(&tokens);
        assert_eq!(
            ink.line_strong,
            to_iced(&signex_types::theme::Color::new(0xDC, 0xD9, 0xD1, 255))
        );
        assert_eq!(ink.err, to_iced(&tokens.error));
        assert_eq!(ink.warn, to_iced(&tokens.warning));
    }
}

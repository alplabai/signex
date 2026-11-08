//! Graphite look of the symbol canvas: the paper (light) or charcoal
//! (dark) ground, dot colour and symbol ink of the Graphite design.

use iced::Color;
use signex_types::theme::ThemeTokens;

use super::SymbolCanvas;

const fn rgb(r: u8, g: u8, b: u8) -> Color {
    Color::from_rgb(r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0)
}

/// Ground, grid dot, symbol ink and text ink of one Graphite canvas.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Ink {
    ground: Color,
    dot: Color,
    sym: Color,
    text: Color,
}

const LIGHT: Ink = Ink {
    ground: rgb(0xFD, 0xFC, 0xFA),
    dot: rgb(0xE2, 0xDF, 0xD8),
    sym: rgb(0x2B, 0x2D, 0x33),
    text: rgb(0x15, 0x17, 0x1A),
};

const DARK: Ink = Ink {
    ground: rgb(0x14, 0x15, 0x18),
    dot: rgb(0x26, 0x28, 0x2D),
    sym: rgb(0xC9, 0xCB, 0xD1),
    text: rgb(0xEC, 0xEA, 0xE5),
};

/// Alpha of the origin crosshair over the ground.
const AXIS_ALPHA: f32 = 0.45;

fn ink_for(tokens: &ThemeTokens) -> Option<Ink> {
    if !crate::styles::is_graphite(tokens) {
        return None;
    }
    let paper = crate::styles::ti(tokens.paper);
    let luma = 0.299 * paper.r + 0.587 * paper.g + 0.114 * paper.b;
    Some(if luma > 0.5 { LIGHT } else { DARK })
}

impl SymbolCanvas<'_> {
    /// Paint with the Graphite canvas colours when `tokens` is a Graphite
    /// theme; any other theme keeps the sheet-colour look unchanged.
    pub fn with_graphite(mut self, tokens: &ThemeTokens) -> Self {
        let Some(ink) = ink_for(tokens) else {
            return self;
        };
        self.bg_color = ink.ground;
        self.grid_color = ink.dot;
        self.body_color = ink.sym;
        self.pin_color = ink.sym;
        self.text_color = ink.text;
        self.axis_color = Color {
            a: AXIS_ALPHA,
            ..ink.sym
        };
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn legacy_tokens_have_no_graphite_ink() {
        let tokens = crate::styles::legacy_tokens_fixture();
        assert!(ink_for(&tokens).is_none());
    }

    #[test]
    fn graphite_light_and_dark_pick_their_own_ground() {
        use signex_types::theme::{ThemeId, theme_tokens};
        assert_eq!(ink_for(&theme_tokens(ThemeId::GraphiteLight)), Some(LIGHT));
        assert_eq!(ink_for(&theme_tokens(ThemeId::GraphiteDark)), Some(DARK));
    }
}

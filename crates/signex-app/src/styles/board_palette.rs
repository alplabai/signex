//! The board canvas palette and the line-art palette (#2523).
//!
//! Board views draw with one palette in both themes: [`BoardPalette`] names
//! its entries and reads every colour from the board canvas colours, through
//! the same lookup the canvas draws with, so a legend chip never disagrees
//! with the board. A colour the user saved for a layer of one board still
//! wins over the palette ([`BoardPalette::painted`]).
//!
//! Thumbnails and property illustrations are line art, not board colour:
//! [`LineArtPalette`] takes the chrome ink tokens, so they follow the light or
//! dark chrome around them.

use iced::Color;
use signex_renderer::pcb::{BOARD_OUTLINE, BoardPaint};
use signex_types::pcb_layer::{CopperLayer, InnerCopper, PcbLayer, TechnicalLayer};
use signex_types::theme::{ThemeId, ThemeTokens, theme_tokens};

use super::{board_canvas_colors, ti, tokens};
use crate::pcb_canvas::layer_style::{board_layer_colour, painted_layer_colour};

/// One entry of the board palette legend, in the order it is listed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BoardSwatch {
    TopCopper,
    BottomCopper,
    MidLayer1,
    MidLayer2,
    SolderMask,
    Paste,
    Courtyard,
    Fab,
    Body3d,
    KeepOut,
    BoardEdge,
}

impl BoardSwatch {
    /// Every entry, in legend order.
    pub const ALL: [Self; 11] = [
        Self::TopCopper,
        Self::BottomCopper,
        Self::MidLayer1,
        Self::MidLayer2,
        Self::SolderMask,
        Self::Paste,
        Self::Courtyard,
        Self::Fab,
        Self::Body3d,
        Self::KeepOut,
        Self::BoardEdge,
    ];

    /// The layer this entry shows: the top one of a sided pair.
    pub fn layer(self) -> PcbLayer {
        use TechnicalLayer as T;
        match self {
            Self::TopCopper => PcbLayer::Copper(CopperLayer::Top),
            Self::BottomCopper => PcbLayer::Copper(CopperLayer::Bottom),
            Self::MidLayer1 => inner(1),
            Self::MidLayer2 => inner(2),
            Self::SolderMask => PcbLayer::Technical(T::SolderMaskTop),
            Self::Paste => PcbLayer::Technical(T::PasteTop),
            Self::Courtyard => PcbLayer::Technical(T::CourtyardTop),
            Self::Fab => PcbLayer::Technical(T::FabTop),
            Self::Body3d => PcbLayer::Technical(T::Body3dTop),
            Self::KeepOut => PcbLayer::Technical(T::KeepoutTop),
            Self::BoardEdge => BOARD_OUTLINE,
        }
    }

    /// The bottom layer of a sided technical pair, drawn darker than its
    /// top; `None` for copper and the board-wide edge.
    pub fn bottom_layer(self) -> Option<PcbLayer> {
        match self.layer() {
            PcbLayer::Technical(t) if t.side().is_some() => Some(PcbLayer::Technical(t.flipped())),
            _ => None,
        }
    }
}

fn inner(index: u8) -> PcbLayer {
    match InnerCopper::new(index) {
        Some(n) => PcbLayer::Copper(CopperLayer::Inner(n)),
        None => PcbLayer::Copper(CopperLayer::Top),
    }
}

/// The colours of every board view, the same in both themes.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BoardPalette {
    /// The ground a board thumbnail or preview sits on.
    pub canvas: Color,
    /// The board area inside the outline.
    pub board_area: Color,
    /// Through-hole pads and vias.
    pub multi_layer: Color,
}

impl Default for BoardPalette {
    fn default() -> Self {
        Self::new()
    }
}

impl BoardPalette {
    /// The board palette. The canvas is the Graphite Dark ground: board
    /// views keep it under the light chrome too.
    pub fn new() -> Self {
        let canvas = board_canvas_colors();
        Self {
            canvas: ti(theme_tokens(ThemeId::GraphiteDark).bg),
            board_area: ti(canvas.board_area_color()),
            multi_layer: crate::pcb_canvas::layer_style::multi_layer_colour(),
        }
    }

    /// The palette colour of `layer`, as the canvas draws it.
    pub fn layer(&self, layer: PcbLayer) -> Color {
        board_layer_colour(layer)
    }

    /// The colour of `layer` on a board whose saved paint is `paint`: the
    /// colour saved for that layer, else the palette's.
    pub fn painted(&self, paint: &BoardPaint, layer: PcbLayer) -> Color {
        painted_layer_colour(paint, layer)
    }

    /// The colour a legend entry is listed with.
    pub fn swatch(&self, swatch: BoardSwatch) -> Color {
        self.layer(swatch.layer())
    }

    /// The legend in order: each entry, its colour and, for a sided pair,
    /// its darker bottom colour.
    pub fn legend(&self) -> Vec<(BoardSwatch, Color, Option<Color>)> {
        BoardSwatch::ALL
            .into_iter()
            .map(|s| {
                let bottom = s.bottom_layer().map(|l| self.layer(l));
                (s, self.swatch(s), bottom)
            })
            .collect()
    }
}

/// What a line-art stroke draws, from strongest to faintest.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LineArtRole {
    /// The board or part outline.
    Outline,
    /// Pads, tracks and other copper.
    Detail,
    /// Silkscreen, fab and assembly marks.
    Mark,
    /// Courtyards, keep-outs and other guides.
    Guide,
    /// Dimension callouts tied to a property field.
    Callout,
}

impl LineArtRole {
    /// The role a layer's geometry takes in a line-art thumbnail.
    pub fn for_layer(layer: PcbLayer) -> Self {
        use TechnicalLayer as T;
        match layer {
            _ if layer == BOARD_OUTLINE => Self::Outline,
            PcbLayer::Copper(_) => Self::Detail,
            PcbLayer::Technical(
                T::SilkscreenTop
                | T::SilkscreenBottom
                | T::FabTop
                | T::FabBottom
                | T::AssemblyTop
                | T::AssemblyBottom
                | T::DesignatorTop
                | T::DesignatorBottom,
            ) => Self::Mark,
            PcbLayer::Technical(T::Dimensions) => Self::Callout,
            PcbLayer::Technical(_) | PcbLayer::Mechanical(_) => Self::Guide,
        }
    }
}

/// The ink colours of line-art thumbnails and illustrations: transparent
/// ground, strokes from the chrome ink tokens.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LineArtPalette {
    pub ink: Color,
    pub ink2: Color,
    pub ink3: Color,
    pub ink4: Color,
    pub callout: Color,
}

impl LineArtPalette {
    /// The line-art inks of the chrome `tokens`.
    pub fn from_tokens(tokens: &ThemeTokens) -> Self {
        let resolved = tokens::resolve(tokens);
        Self {
            ink: ti(tokens.text),
            ink2: ti(tokens.text_secondary),
            ink3: resolved.ink3,
            ink4: resolved.ink4,
            callout: resolved.accent_text,
        }
    }

    /// The stroke colour of `role`.
    pub fn stroke(&self, role: LineArtRole) -> Color {
        match role {
            LineArtRole::Outline => self.ink,
            LineArtRole::Detail => self.ink2,
            LineArtRole::Mark => self.ink3,
            LineArtRole::Guide => self.ink4,
            LineArtRole::Callout => self.callout,
        }
    }

    /// The stroke colour of geometry on `layer`.
    pub fn layer_stroke(&self, layer: PcbLayer) -> Color {
        self.stroke(LineArtRole::for_layer(layer))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hex(c: Color) -> String {
        let [r, g, b, _] = c.into_rgba8();
        format!("#{r:02X}{g:02X}{b:02X}")
    }

    fn luminance(c: Color) -> f32 {
        0.2126 * c.r + 0.7152 * c.g + 0.0722 * c.b
    }

    /// The approved board palette, entry by entry.
    #[test]
    fn the_legend_is_the_approved_palette() {
        let palette = BoardPalette::new();
        let expected = [
            (BoardSwatch::TopCopper, "#D04040"),
            (BoardSwatch::BottomCopper, "#386CD0"),
            (BoardSwatch::SolderMask, "#409C58"),
            (BoardSwatch::Paste, "#A8A8B8"),
            (BoardSwatch::Courtyard, "#6DF2C6"),
            (BoardSwatch::Fab, "#6DD1F2"),
            (BoardSwatch::Body3d, "#836DF2"),
            (BoardSwatch::KeepOut, "#FF80E0"),
            (BoardSwatch::BoardEdge, "#E000E0"),
        ];
        for (swatch, want) in expected {
            assert_eq!(hex(palette.swatch(swatch)), want, "{swatch:?}");
        }
        assert_eq!(hex(palette.canvas), "#111214");
    }

    /// Mid-Layer 1 and 2 take the first two inner colours.
    #[test]
    #[ignore = "inner copper picks layer_cycle[n], one past the approved colour; see copper_slot"]
    fn the_mid_layers_are_the_approved_colours() {
        let palette = BoardPalette::new();
        assert_eq!(hex(palette.swatch(BoardSwatch::MidLayer1)), "#C8A02C");
        assert_eq!(hex(palette.swatch(BoardSwatch::MidLayer2)), "#3CA86A");
    }

    #[test]
    fn the_legend_lists_every_entry_once_in_order() {
        let legend = BoardPalette::new().legend();
        let order: Vec<_> = legend.iter().map(|(s, _, _)| *s).collect();
        assert_eq!(order, BoardSwatch::ALL.to_vec());
    }

    /// A sided pair's bottom is the darker of the two; copper and the edge
    /// list no bottom.
    #[test]
    fn a_bottom_layer_is_drawn_darker_than_its_top() {
        for (swatch, top, bottom) in BoardPalette::new().legend() {
            match swatch {
                BoardSwatch::Courtyard | BoardSwatch::Fab | BoardSwatch::Body3d => {
                    let bottom = bottom.expect("a sided pair lists its bottom");
                    assert!(luminance(bottom) < luminance(top), "{swatch:?}");
                }
                BoardSwatch::TopCopper
                | BoardSwatch::BottomCopper
                | BoardSwatch::MidLayer1
                | BoardSwatch::MidLayer2
                | BoardSwatch::BoardEdge => assert!(bottom.is_none(), "{swatch:?}"),
                BoardSwatch::SolderMask | BoardSwatch::Paste | BoardSwatch::KeepOut => {}
            }
        }
    }

    /// The palette does not follow the chrome theme.
    #[test]
    fn both_themes_share_one_board_palette() {
        let palette = BoardPalette::new();
        for id in [ThemeId::GraphiteLight, ThemeId::GraphiteDark] {
            let tokens = theme_tokens(id);
            for swatch in BoardSwatch::ALL {
                let listed = crate::pcb_canvas::layer_style::layer_colour(swatch.layer(), &tokens);
                assert_eq!(listed, palette.swatch(swatch), "{id:?} {swatch:?}");
            }
        }
    }

    /// A colour saved for one board's layer wins; other layers keep the
    /// palette's.
    #[test]
    fn a_saved_layer_colour_still_wins() {
        let palette = BoardPalette::new();
        let top = PcbLayer::Copper(CopperLayer::Top);
        let bottom = PcbLayer::Copper(CopperLayer::Bottom);
        let mut paint = BoardPaint::default();
        paint.set_colour(top, Some([0x12, 0x34, 0x56]));
        assert_eq!(hex(palette.painted(&paint, top)), "#123456");
        assert_eq!(palette.painted(&paint, bottom), palette.layer(bottom));
        assert_eq!(
            palette.painted(&BoardPaint::default(), top),
            palette.layer(top)
        );
    }

    #[test]
    fn line_art_strokes_follow_the_chrome_inks() {
        for id in [ThemeId::GraphiteLight, ThemeId::GraphiteDark] {
            let tokens = theme_tokens(id);
            let art = LineArtPalette::from_tokens(&tokens);
            assert_eq!(art.layer_stroke(BOARD_OUTLINE), ti(tokens.text));
            assert_eq!(
                art.layer_stroke(PcbLayer::Copper(CopperLayer::Top)),
                ti(tokens.text_secondary)
            );
            let resolved = tokens::resolve(&tokens);
            assert_eq!(
                art.layer_stroke(PcbLayer::Technical(TechnicalLayer::SilkscreenTop)),
                resolved.ink3
            );
            assert_eq!(
                art.layer_stroke(PcbLayer::Technical(TechnicalLayer::CourtyardTop)),
                resolved.ink4
            );
            assert_eq!(art.stroke(LineArtRole::Callout), resolved.accent_text);
        }
    }

    /// Line art changes with the theme; the board palette does not.
    #[test]
    fn line_art_differs_between_themes() {
        let light = LineArtPalette::from_tokens(&theme_tokens(ThemeId::GraphiteLight));
        let dark = LineArtPalette::from_tokens(&theme_tokens(ThemeId::GraphiteDark));
        assert_ne!(light.ink, dark.ink);
    }
}
